//! Redis-backed session store implementing the `SessionStore` trait.
//!
//! Sessions are stored as JSON strings with a 24-hour TTL under the key
//! `session:{uuid}`. User-session lookups (for `delete_all_user_sessions`)
//! use `SCAN` to iterate over all session keys.

use async_trait::async_trait;
use chrono::Utc;
use redis::AsyncCommands;
use redis::aio::MultiplexedConnection;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::application::services::errors::ServiceError;
use crate::application::services::session_store::SessionStore;
use crate::domain::entities::session::Session;

/// Redis-backed implementation of [`SessionStore`].
///
/// # Key format
///
/// Each session is stored as:
///
/// ```text
/// session:{session_id}  →  SETEX 86400  '{"session_id":"...","user_id":..., ...}'
/// ```
///
/// The TTL is 86 400 seconds (24 hours).
///
/// # Concurrency
///
/// A single [`MultiplexedConnection`] is shared behind a [`Mutex`]. Each
/// operation locks the connection, issues its commands, and releases it.
/// This avoids blocking the async runtime on synchronous `get_connection`
/// calls and is suitable for moderate throughput.
pub struct RedisSessionStore {
    connection: Mutex<MultiplexedConnection>,
}

impl RedisSessionStore {
    /// Create a new `RedisSessionStore` from a multiplexed async connection.
    pub fn new(connection: MultiplexedConnection) -> Self {
        Self {
            connection: Mutex::new(connection),
        }
    }
}

#[async_trait]
impl SessionStore for RedisSessionStore {
    async fn create_session(
        &self,
        user_id: i64,
        fingerprint: Option<&str>,
    ) -> Result<Session, ServiceError> {
        let session = Session::new(user_id, fingerprint.map(String::from));

        let json =
            serde_json::to_string(&session).map_err(|e| ServiceError::Cache(e.to_string()))?;

        let key = format!("session:{}", session.session_id);
        let ttl: u64 = 86_400; // 24 hours

        let mut conn = self.connection.lock().await;
        conn.set_ex::<_, _, ()>(&key, &json, ttl)
            .await
            .map_err(|e| ServiceError::Cache(format!("failed to store session in Redis: {}", e)))?;

        Ok(session)
    }

    async fn get_session(&self, session_id: &Uuid) -> Result<Option<Session>, ServiceError> {
        let key = format!("session:{}", session_id);

        let result: Option<String> = {
            let mut conn = self.connection.lock().await;
            conn.get(&key).await.map_err(|e| {
                ServiceError::Cache(format!("failed to read session from Redis: {}", e))
            })?
        };

        match result {
            Some(json) => {
                let session: Session =
                    serde_json::from_str(&json).map_err(|e| ServiceError::Cache(e.to_string()))?;

                // Absolute maximum session lifetime: 7 days.
                // After this, sliding expiration stops and the session will
                // expire naturally at the end of its remaining Redis TTL.
                // Within the 7-day window, the TTL is refreshed to 24 hours
                // on each access (sliding expiration).
                const MAX_SESSION_LIFETIME: i64 = 7 * 24 * 3600;
                let age_seconds = Utc::now().timestamp() - session.created_at.timestamp();
                if age_seconds < MAX_SESSION_LIFETIME {
                    let mut conn = self.connection.lock().await;
                    redis::cmd("EXPIRE")
                        .arg(&key)
                        .arg(86_400u64)
                        .query_async::<()>(&mut *conn)
                        .await
                        .map_err(|e| {
                            ServiceError::Cache(format!("failed to refresh session TTL: {}", e))
                        })?;
                }

                Ok(Some(session))
            }
            None => Ok(None),
        }
    }

    async fn delete_session(&self, session_id: &Uuid) -> Result<(), ServiceError> {
        let key = format!("session:{}", session_id);

        let mut conn = self.connection.lock().await;
        conn.del::<_, ()>(&key).await.map_err(|e| {
            ServiceError::Cache(format!("failed to delete session from Redis: {}", e))
        })?;

        Ok(())
    }

    async fn delete_all_user_sessions(&self, user_id: i64) -> Result<u64, ServiceError> {
        // Phase 1: SCAN and collect keys that belong to the target user.
        // The lock is acquired and released for each Redis operation so that
        // other callers are not blocked for the duration of a full SCAN sweep.
        // -------------------------------------------------------------------
        let mut cursor: u64 = 0;
        let mut keys_to_delete: Vec<String> = Vec::new();

        loop {
            let (next_cursor, keys): (u64, Vec<String>) = {
                let mut conn = self.connection.lock().await;
                redis::cmd("SCAN")
                    .arg(cursor)
                    .arg("MATCH")
                    .arg("session:*")
                    .arg("COUNT")
                    .arg(100)
                    .query_async(&mut *conn)
                    .await
                    .map_err(|e| ServiceError::Cache(format!("Redis SCAN failed: {}", e)))?
            };

            // Check each key's content to see if it belongs to this user.
            // Lock is acquired and released for each GET individually.
            for key in &keys {
                let value: Option<String> = {
                    let mut conn = self.connection.lock().await;
                    redis::cmd("GET")
                        .arg(key)
                        .query_async(&mut *conn)
                        .await
                        .map_err(|e| {
                            ServiceError::Cache(format!(
                                "failed to read session key '{}': {}",
                                key, e
                            ))
                        })?
                };

                if let Some(json) = value
                    && let Ok(session) = serde_json::from_str::<Session>(&json)
                    && session.user_id == user_id
                {
                    keys_to_delete.push(key.clone());
                }
            }

            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }

        // Phase 2: Bulk-delete the matching keys.
        // Lock is held only for the DEL command itself.
        // -------------------------------------------------------------------
        if keys_to_delete.is_empty() {
            return Ok(0);
        }

        let count: u64 = {
            let mut conn = self.connection.lock().await;
            redis::cmd("DEL")
                .arg(&keys_to_delete)
                .query_async(&mut *conn)
                .await
                .map_err(|e| {
                    ServiceError::Cache(format!("failed to delete user sessions: {}", e))
                })?
        };

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    /// Helper to create a store pointed at a local Redis instance.
    ///
    /// Returns `None` when Redis is not available so that tests gracefully
    /// skip rather than panic on machines without a running Redis server.
    async fn test_store() -> Option<RedisSessionStore> {
        let client = redis::Client::open("redis://127.0.0.1:6379").ok()?;
        let conn = client.get_multiplexed_async_connection().await.ok()?;
        Some(RedisSessionStore::new(conn))
    }

    #[tokio::test]
    async fn create_and_get_session() {
        let Some(store) = test_store().await else { return; };
        let session = store
            .create_session(42, Some("browser-fingerprint"))
            .await
            .expect("create_session should succeed");

        let fetched = store
            .get_session(&session.session_id)
            .await
            .expect("get_session should succeed")
            .expect("session should exist");

        assert_eq!(fetched.user_id, 42);
        assert_eq!(fetched.fingerprint, Some("browser-fingerprint".to_string()));
    }

    #[tokio::test]
    async fn get_nonexistent_session() {
        let Some(store) = test_store().await else { return; };
        let result = store
            .get_session(&Uuid::new_v4())
            .await
            .expect("get_session should succeed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn delete_session() {
        let Some(store) = test_store().await else { return; };
        let session = store
            .create_session(7, None)
            .await
            .expect("create_session should succeed");

        store
            .delete_session(&session.session_id)
            .await
            .expect("delete_session should succeed");

        let fetched = store
            .get_session(&session.session_id)
            .await
            .expect("get_session should succeed");
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn delete_all_user_sessions() {
        let Some(store) = test_store().await else { return; };

        // Create two sessions for user 1 and one for user 2.
        let s1 = store.create_session(1, None).await.unwrap();
        let s2 = store.create_session(1, None).await.unwrap();
        let _s3 = store.create_session(2, None).await.unwrap();

        let deleted = store
            .delete_all_user_sessions(1)
            .await
            .expect("delete_all_user_sessions should succeed");
        assert_eq!(deleted, 2);

        // Verify user 1's sessions are gone.
        assert!(store.get_session(&s1.session_id).await.unwrap().is_none());
        assert!(store.get_session(&s2.session_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_all_user_sessions_noop() {
        let Some(store) = test_store().await else { return; };
        let deleted = store
            .delete_all_user_sessions(999)
            .await
            .expect("delete_all_user_sessions should succeed");
        assert_eq!(deleted, 0);
    }
}
