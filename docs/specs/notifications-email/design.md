# Design: Email & In-App Notifications

## Architecture

The Notifications feature follows Clean Architecture layering. It introduces a
domain event-driven side effect: when a test case result status changes, the
system creates an in-app notification and sends an email. The notification
endpoints themselves are straightforward CRUD-like read and update operations.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                        │   │
│  │  - list_notifications      GET    /api/v1/notifications              │   │
│  │  - mark_read               PATCH  /api/v1/notifications/{id}/read    │   │
│  │  - mark_all_read           POST   /api/v1/notifications/read-all     │   │
│  │                                                                        │   │
│  │  Event Listener (hooked into result update flow):                     │   │
│  │  - on_result_status_changed(event) → NotificationService              │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  NotificationService:                     │  │  - Notification (entity)│  │
│  │  - list_notifications                     │  │  - NotificationType (vo)│  │
│  │  - mark_as_read                           │  └──────────────────────────┘  │
│  │  - mark_all_as_read                       │                                │
│  │  - send_result_change_notification        │                                │
│  │                                           │                                │
│  │  Interfaces:                              │                                │
│  │  - NotificationRepository (port)          │                                │
│  │  - EmailSender (port)                     │                                │
│  │  - NotificationCoalescer (port)           │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlNotificationRepository (implements NotificationRepository)       │   │
│  │  - SmtpEmailSender          (implements EmailSender via SMTP)          │   │
│  │  - RedisNotificationCoalescer (implements NotificationCoalescer)       │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. A test case result status is updated via the execution management feature.
2. The update handler (or service) emits a `ResultStatusChanged` domain event
   containing the test case result, old status, new status, and the updating user.
3. A `NotificationService` event listener receives the event and determines:
   a. Who is the `report_to` user for the ancestor test run.
   b. Whether the report_to user is valid (exists, active, not the updater).
   c. Whether the notification should be coalesced (duplicate within 60s window).
4. If conditions are met, the service creates an in-app notification via
   `NotificationRepository` and sends an email via `EmailSender`.
5. Email sending is best-effort: failure does not roll back the in-app notification.
6. Notification read/list endpoints are straightforward CRUD on the NOTIFICATIONS
   table, scoped to the authenticated user.

---

## API Contract

### Common Error Response Format

All errors follow the standard format:

```json
{
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable description",
    "details": [
      { "field": "field_name", "message": "specific validation message" }
    ]
  }
}
```

---

### GET `/api/v1/notifications`

List notifications for the authenticated user.

**Authentication:** Required (session)

**Query Parameters:**

| Param | Type | Default | Max | Description |
|-------|------|---------|-----|-------------|
| `page` | integer | 1 | -- | Page number (1-based) |
| `limit` | integer | 25 | 100 | Items per page |
| `is_read` | boolean | -- | -- | Optional filter: `true` or `false` |
| `type` | string | -- | -- | Optional filter: `TEST_CASE_RESULT_CHANGE` |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "id": 1,
      "type": "TEST_CASE_RESULT_CHANGE",
      "title": "Result changed to FAIL for TC-42: Login with invalid password",
      "body": "The test case result for 'Login with invalid password' in test run 'Sprint 12 Regression' was changed from IN PROGRESS to FAIL by jdoe.",
      "is_read": false,
      "related_object_type": "test_case_result",
      "related_object_id": 105,
      "created_at": "2026-07-15T10:30:00Z"
    }
  ],
  "meta": {
    "total": 15,
    "page": 1,
    "limit": 25
  }
}
```

**Notes:**
- Results are scoped to the authenticated user only.
- Ordered by `created_at DESC`.
- No cross-user access -- the `user_id` filter is derived from the session.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `422` | `VALIDATION_ERROR` | Invalid query parameter value |

---

### PATCH `/api/v1/notifications/{id}/read`

Mark a single notification as read.

**Authentication:** Required (session)

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Notification ID |

**Request Body:** None

**Success Response:** `200 OK`

```json
{
  "data": {
    "id": 1,
    "type": "TEST_CASE_RESULT_CHANGE",
    "title": "Result changed to FAIL for TC-42: Login with invalid password",
    "body": "...",
    "is_read": true,
    "related_object_type": "test_case_result",
    "related_object_id": 105,
    "created_at": "2026-07-15T10:30:00Z"
  }
}
```

**Notes:**
- Idempotent: marking an already-read notification returns `200 OK` (not an error).
- Cross-user check: if the notification's `user_id` does not match the
  authenticated user, return `403 Forbidden`.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | Notification belongs to a different user |
| `404` | `NOT_FOUND` | Notification does not exist |

---

### POST `/api/v1/notifications/read-all`

Mark all unread notifications as read for the authenticated user.

**Authentication:** Required (session)

**Request Body:** None

**Success Response:** `200 OK`

```json
{
  "data": {
    "count": 12
  }
}
```

**Notes:**
- Idempotent: if there are no unread notifications, returns `count = 0`.
- Scoped to the authenticated user only.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |

---

## Data Model

### New Tables

#### NOTIFICATIONS

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | `BIGINT` | `PK`, `GENERATED ALWAYS AS IDENTITY` | Surrogate primary key |
| `user_id` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE CASCADE` | Recipient user |
| `type` | `VARCHAR(50)` | `NOT NULL` | Notification type: `TEST_CASE_RESULT_CHANGE` |
| `title` | `VARCHAR(500)` | `NOT NULL` | Notification title / email subject |
| `body` | `TEXT` | `NOT NULL` | Notification body / email body |
| `is_read` | `BOOLEAN` | `NOT NULL`, `DEFAULT FALSE` | Whether the user has read the notification |
| `related_object_type` | `VARCHAR(50)` | `NOT NULL` | Type of the related entity, e.g., `test_case_result` |
| `related_object_id` | `BIGINT` | `NOT NULL` | ID of the related entity |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | When the notification was created |

**Constraints:**

```sql
-- Check constraint on notification type
ALTER TABLE notifications ADD CONSTRAINT chk_notifications_type
  CHECK (type IN ('TEST_CASE_RESULT_CHANGE'));

-- Index for user-scoped list queries (most common query pattern)
CREATE INDEX idx_notifications_user_unread
  ON notifications (user_id, created_at DESC) WHERE is_read = FALSE;

-- Index for user-scoped list queries (all notifications)
CREATE INDEX idx_notifications_user_created
  ON notifications (user_id, created_at DESC);

-- Index for mark-all-read bulk updates
CREATE INDEX idx_notifications_user_unread_all
  ON notifications (user_id) WHERE is_read = FALSE;
```

**Design notes:**
- No `updated_at` or `updated_by` columns -- notifications are immutable after
  creation except for the `is_read` flag.
- No `deleted_at` or `deleted_by` -- notifications are retained indefinitely.
  A cleanup policy may be added in a future phase to prune old notifications.
- `ON DELETE CASCADE` on `user_id` FK: if a user is deleted, their notifications
  are cleaned up.
- The `related_object_type` + `related_object_id` pair allows linking notifications
  to the triggering entity without a polymorphic FK. Clients can construct a
  navigation URL from this pair.

---

## Sequence

### Notification on Result Status Change

1. Test case result update completes successfully (status changed from old_value
   to new_value).
2. The execution service (or domain entity) emits a `ResultStatusChanged` domain
   event: `{ result_id, test_case_id, test_case_summary, test_run_id,
   test_run_summary, old_status, new_status, updated_by_user_id,
   updated_by_username }`.
3. `NotificationService::on_result_status_changed(event)` is invoked.
4. **Guard: No change** -- if `old_status == new_status`, return (no-op).
5. **Guard: Notification recipient** -- the service looks up the `report_to`
   user_id from the test run:
   a. If `report_to` is NULL, the user does not exist, the user is soft-deleted,
      or the user is INACTIVE, log a warning and return (no notification).
   b. If `report_to` == `updated_by_user_id`, return (self-notification
      suppressed).
6. **Coalescing check** -- `NotificationCoalescer::should_send(test_case_result_id,
   new_status, updated_by_user_id)`:
   a. Store a Redis key: `coalesce:result:<result_id>:<new_status>:<user_id>` with
      TTL of 60 seconds (configurable).
   b. If the key already exists, return `false` (duplicate suppressed).
   c. If the key does not exist, set it and return `true`.
7. If coalescing check returns `false`, log DEBUG and return (no-op).
8. **Create in-app notification** -- `NotificationRepository::save(notification)`:
   a. Construct title: `"Result changed to {new_status} for TC-{test_case_id}:
      {test_case_summary}"`.
   b. Construct body: `"The test case result for '{test_case_summary}' in test
      run '{test_run_summary}' was changed from {old_status} to {new_status} by
      {updated_by_username}."`.
   c. Insert into `NOTIFICATIONS` table.
9. **Send email** (best-effort) -- `EmailSender::send(to_email, subject, body)`:
   a. Look up the `report_to` user's email from the USERS table.
   b. Send email via SMTP.
   c. If SMTP fails, log ERROR with the failure reason. Do not throw -- the
      in-app notification is already persisted.

### List Notifications Flow

1. Client sends `GET /api/v1/notifications?page=1&limit=25&is_read=false`.
2. `AuthMiddleware` validates the session and attaches user context.
3. Handler calls `NotificationService::list_notifications(user_id, query)`.
4. Service calls `NotificationRepository::find_by_user_id(user_id, page, limit,
   is_read_filter, type_filter)`.
5. Repository executes query scoped to `user_id`, ordered by `created_at DESC`.
6. Handler returns paginated response.

### Mark Read Flow

1. Client sends `PATCH /api/v1/notifications/{id}/read`.
2. Handler calls `NotificationService::mark_as_read(notification_id, user_id)`.
3. Service calls `NotificationRepository::find_by_id(notification_id)`.
4. If not found, return `NotFoundError`.
5. If `notification.user_id != user_id`, return `ForbiddenError`.
6. Service calls `NotificationRepository::mark_as_read(notification_id)` --
   idempotent (no change if already read).
7. Handler returns updated notification.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `Notification` | Domain (1) | Entity: `id`, `user_id`, `type`, `title`, `body`, `is_read`, `related_object_type`, `related_object_id`, `created_at`. Factory method `create(user_id, notif_type, title, body, related_type, related_id)` with validation (title not empty, type valid). |
| `NotificationType` | Domain (1) | Enum: `Test case ResultChange`. Maps to the `type` column. |
| `ResultStatusChanged` | Domain (1) | Domain event struct: carries old/new status, test case info, test run info, updater info. Not persisted itself -- triggers the notification side effect. |
| `NotificationService` | Application (2) | Orchestrates: `list_notifications`, `mark_as_read`, `mark_all_as_read`, `on_result_status_changed`. The last method is the event handler. |
| `NotificationRepository` | Application (2) | Interface (port): `save`, `find_by_id`, `find_by_user_id` (paginated, filtered), `mark_as_read`, `mark_all_as_read`. |
| `EmailSender` | Application (2) | Interface (port): `send_email(to, subject, body)`. Best-effort delivery. |
| `NotificationCoalescer` | Application (2) | Interface (port): `should_send(result_id, new_status, user_id) -> bool`. Prevents duplicate notifications within a time window. |
| `NotificationHandler` | Adapters (3) | HTTP handler: `list`, `mark_read`, `mark_all_read` methods. |
| `SqlNotificationRepository` | Infrastructure (4) | Implements `NotificationRepository` using SQLx/Diesel with parameterized queries. |
| `SmtpEmailSender` | Infrastructure (4) | Implements `EmailSender` using SMTP. Configured via environment variables. |
| `RedisNotificationCoalescer` | Infrastructure (4) | Implements `NotificationCoalescer` using Redis `SET key value EX ttl NX` (set-if-not-exists with TTL). |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| Test case result update handler/service | After persisting a status change, emit the `ResultStatusChanged` event and dispatch to `NotificationService::on_result_status_changed`. The event dispatch must happen after the transaction commits (post-commit hook) to ensure the notification is only sent if the update was successful. |
| HTTP router registration | Register three new notification routes under `/api/v1/notifications/` -- all require session auth. |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| Notification belongs to different user | `403` | `FORBIDDEN` | INFO | Generic message; do not reveal notification existence |
| Notification not found | `404` | `NOT_FOUND` | INFO | |
| Invalid query parameter | `422` | `VALIDATION_ERROR` | INFO | Field-level details in response |
| SMTP unreachable / email send failure | N/A (in-app notification already persisted) | N/A | ERROR | Logged server-side only; no client-facing error |
| Notification coalescing Redis failure | N/A (falls through to always-send) | N/A | WARN | Coalescing is an optimization, not a correctness requirement. On Redis failure, always send the notification. |
| Report-to user invalid (deleted, inactive, null) | N/A | N/A | WARN | Skips notification silently; logged server-side |
| Self-notification (report_to == updater) | N/A | N/A | DEBUG | Skips notification silently |

**Anti-patterns explicitly avoided:**

- **Do not** send an email inside the result update transaction. Email is sent
  after the transaction commits.
- **Do not** return an error to the client if email delivery fails. The in-app
  notification is the system of record.
- **Do not** allow cross-user notification access. The `user_id` check on
  mark-read is mandatory.
- **Do not** use string interpolation for SQL. All queries use parameterized
  placeholders (`$1`, `$2`).
- **Do not** store the plaintext email address in the NOTIFICATIONS table -- it
  is looked up from the USERS table at send time.

---

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `SMTP_HOST` | `localhost` | SMTP server hostname |
| `SMTP_PORT` | `587` | SMTP server port |
| `SMTP_USERNAME` | -- | SMTP authentication username |
| `SMTP_PASSWORD` | -- | SMTP authentication password |
| `SMTP_USE_TLS` | `true` | Use STARTTLS for SMTP connection |
| `SMTP_FROM_ADDRESS` | `noreply@hoa-tcms.local` | From address for notification emails |
| `SMTP_FROM_NAME` | `HOA TCMS` | From display name |
| `NOTIFICATION_COALESCE_SECONDS` | `60` | Duplicate notification suppression window |
| `EMAIL_MAX_RETRIES` | `3` | Number of retries for failed email delivery |
| `EMAIL_RETRY_DELAY_MS` | `1000` | Delay between email retry attempts |
