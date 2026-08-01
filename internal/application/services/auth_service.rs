//! `AuthService` — authentication use cases (login, logout).
//!
//! Wires the `UserRepository`, `PasswordHasher`, and `SessionStore` ports
//! together to authenticate a user and establish an HTTP session.

use std::sync::Arc;
use uuid::Uuid;

use crate::application::repositories::user_repository::UserRepository;
use crate::application::services::errors::ServiceError;
use crate::application::services::password_hasher::PasswordHasher;
use crate::application::services::session_store::SessionStore;
use crate::domain::entities::session::Session;
use crate::domain::entities::user::User;

/// Authentication use cases.
pub struct AuthService {
    user_repo: Box<dyn UserRepository>,
    password_hasher: Arc<dyn PasswordHasher>,
    session_store: Arc<dyn SessionStore>,
}

impl AuthService {
    /// Create a new `AuthService` with its dependencies.
    pub fn new(
        user_repo: Box<dyn UserRepository>,
        password_hasher: Arc<dyn PasswordHasher>,
        session_store: Arc<dyn SessionStore>,
    ) -> Self {
        Self {
            user_repo,
            password_hasher,
            session_store,
        }
    }

    /// Authenticate a user by username or email and create a session.
    ///
    /// Returns the authenticated [`User`] and the newly created [`Session`]
    /// on success. Fails with a generic error for both unknown credentials
    /// and wrong passwords so the endpoint does not leak which usernames
    /// exist (prevents username enumeration).
    pub async fn login(
        &self,
        username_or_email: &str,
        password: &str,
        fingerprint: Option<&str>,
    ) -> Result<(User, Session), ServiceError> {
        let ident = username_or_email.trim();
        if ident.is_empty() || password.is_empty() {
            return Err(ServiceError::Validation("credentials are required".into()));
        }

        let user = match self.user_repo.find_by_username(ident).await? {
            Some(u) => u,
            None => match self.user_repo.find_by_email(ident).await? {
                Some(u) => u,
                None => return Err(ServiceError::NotFound),
            },
        };

        if !self.password_hasher.verify(password, &user.password_hash) {
            return Err(ServiceError::NotFound);
        }

        if !user.is_active() {
            return Err(ServiceError::PermissionDenied(
                "account is not active".into(),
            ));
        }

        let session = self
            .session_store
            .create_session(user.id, fingerprint)
            .await?;
        Ok((user, session))
    }

    /// Destroy a session (used on logout). A no-op if the session is unknown.
    pub async fn logout(&self, session_id: &Uuid) -> Result<(), ServiceError> {
        self.session_store.delete_session(session_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::repositories::RepositoryResult;
    use crate::domain::entities::session::Session;
    use crate::domain::entities::user::User;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// In-memory `UserRepository` for tests.
    struct MockUserRepo {
        users: Vec<User>,
    }

    #[async_trait::async_trait]
    impl UserRepository for MockUserRepo {
        async fn find_by_id(&self, id: i64) -> RepositoryResult<Option<User>> {
            Ok(self.users.iter().find(|u| u.id == id).cloned())
        }
        async fn find_by_username(&self, username: &str) -> RepositoryResult<Option<User>> {
            Ok(self.users.iter().find(|u| u.username == username).cloned())
        }
        async fn find_by_email(&self, email: &str) -> RepositoryResult<Option<User>> {
            Ok(self.users.iter().find(|u| u.email == email).cloned())
        }
        async fn create(&self, user: &User) -> RepositoryResult<User> {
            Ok(user.clone())
        }
        async fn update(&self, user: &User) -> RepositoryResult<User> {
            Ok(user.clone())
        }
        async fn soft_delete(&self, _id: i64, _deleted_by: i64) -> RepositoryResult<()> {
            Ok(())
        }
        async fn list_paginated(
            &self,
            _page: u32,
            _limit: u32,
        ) -> RepositoryResult<(Vec<User>, u64)> {
            Ok((self.users.clone(), self.users.len() as u64))
        }
        async fn update_password_hash(
            &self,
            _id: i64,
            _password_hash: &str,
        ) -> RepositoryResult<()> {
            Ok(())
        }
    }

    /// `PasswordHasher` where the "hash" is the plaintext password.
    struct MockHasher;

    #[async_trait::async_trait]
    impl PasswordHasher for MockHasher {
        async fn hash(&self, password: &str) -> Result<String, String> {
            Ok(password.to_string())
        }
        fn verify(&self, password: &str, hash: &str) -> bool {
            password == hash
        }
    }

    /// In-memory `SessionStore` for tests.
    #[derive(Default)]
    struct MockSessionStore {
        sessions: Mutex<HashMap<Uuid, Session>>,
    }

    #[async_trait::async_trait]
    impl SessionStore for MockSessionStore {
        async fn create_session(
            &self,
            user_id: i64,
            fingerprint: Option<&str>,
        ) -> Result<Session, ServiceError> {
            let session = Session::new(user_id, fingerprint.map(String::from));
            self.sessions
                .lock()
                .unwrap()
                .insert(session.session_id, session.clone());
            Ok(session)
        }
        async fn get_session(&self, session_id: &Uuid) -> Result<Option<Session>, ServiceError> {
            Ok(self.sessions.lock().unwrap().get(session_id).cloned())
        }
        async fn delete_session(&self, session_id: &Uuid) -> Result<(), ServiceError> {
            self.sessions.lock().unwrap().remove(session_id);
            Ok(())
        }
        async fn delete_all_user_sessions(&self, user_id: i64) -> Result<u64, ServiceError> {
            let mut map = self.sessions.lock().unwrap();
            let before = map.len();
            map.retain(|_, s| s.user_id != user_id);
            Ok((before - map.len()) as u64)
        }
    }

    fn service_with(users: Vec<User>) -> AuthService {
        AuthService::new(
            Box::new(MockUserRepo { users }),
            Arc::new(MockHasher),
            Arc::new(MockSessionStore::default()),
        )
    }

    fn active_user(username: &str, email: &str) -> User {
        User::create(
            username.to_string(),
            email.to_string(),
            "secret".into(),
            "Test User".into(),
            None,
        )
    }

    #[tokio::test]
    async fn login_by_username_succeeds() {
        let service = service_with(vec![active_user("alice", "alice@example.com")]);
        let (user, session) = service
            .login("alice", "secret", None)
            .await
            .expect("login ok");
        assert_eq!(user.username, "alice");
        assert_eq!(session.user_id, user.id);
    }

    #[tokio::test]
    async fn login_by_email_succeeds() {
        let service = service_with(vec![active_user("alice", "alice@example.com")]);
        let (user, _) = service
            .login("alice@example.com", "secret", Some("fp"))
            .await
            .expect("login by email ok");
        assert_eq!(user.username, "alice");
    }

    #[tokio::test]
    async fn login_rejects_wrong_password() {
        let service = service_with(vec![active_user("alice", "alice@example.com")]);
        let err = service.login("alice", "wrong", None).await.unwrap_err();
        assert!(matches!(err, ServiceError::NotFound));
    }

    #[tokio::test]
    async fn login_rejects_unknown_user() {
        let service = service_with(vec![active_user("alice", "alice@example.com")]);
        let err = service.login("nobody", "secret", None).await.unwrap_err();
        assert!(matches!(err, ServiceError::NotFound));
    }

    #[tokio::test]
    async fn login_rejects_inactive_user() {
        let mut user = active_user("alice", "alice@example.com");
        user.status = crate::domain::value_objects::user_status::UserStatus::Inactive;
        let service = service_with(vec![user]);
        let err = service.login("alice", "secret", None).await.unwrap_err();
        assert!(matches!(err, ServiceError::PermissionDenied(_)));
    }

    #[tokio::test]
    async fn logout_removes_session() {
        let store = Arc::new(MockSessionStore::default());
        let service = AuthService::new(
            Box::new(MockUserRepo {
                users: vec![active_user("bob", "bob@example.com")],
            }),
            Arc::new(MockHasher),
            store.clone(),
        );
        let (_, session) = service.login("bob", "secret", None).await.unwrap();
        service.logout(&session.session_id).await.unwrap();
        assert!(
            store
                .get_session(&session.session_id)
                .await
                .unwrap()
                .is_none()
        );
    }
}
