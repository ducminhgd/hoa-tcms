# Tasks: Email & In-App Notifications

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable
unit of work that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Implement `NotificationType` enum --
       `requirements.md#US-1`, `design.md#Components`
  - Variant: `Test case ResultChange`
  - Maps to the `type` column in NOTIFICATIONS table
  - No framework imports; pure Rust enum

- [ ] 2. Implement `Notification` entity --
       `requirements.md#US-1`, `design.md#Components`
  - Fields: `id`, `user_id`, `type` (NotificationType), `title`, `body`,
    `is_read`, `related_object_type`, `related_object_id`, `created_at`
  - Factory method `Notification::create(user_id, notif_type, title, body,
    related_type, related_id)` with validation: title not empty, type valid,
    body not empty
  - No ORM / framework imports

- [ ] 3. Define `ResultStatusChanged` domain event --
       `requirements.md#US-1`, `design.md#Sequence`
  - Fields: `result_id`, `test_case_id`, `test_case_summary`, `test_run_id`,
    `test_run_summary`, `old_status`, `new_status`, `updated_by_user_id`,
    `updated_by_username`
  - Simple data struct; no behaviour

---

## Layer 2 -- Application

- [ ] 4. Define `NotificationRepository` interface (port) --
       `design.md#Components`, `design.md#Data Model`
  - Methods: `save(notification)`, `find_by_id(id)`,
    `find_by_user_id(user_id, page, limit, is_read, type_filter)` (paginated,
    filtered), `mark_as_read(id)`, `mark_all_as_read(user_id)`
  - All methods return domain entities (not DTOs)
  - `mark_as_read` and `mark_all_as_read` are idempotent

- [ ] 5. Define `EmailSender` interface (port) --
       `design.md#Components`, `requirements.md#US-1`
  - Method: `async fn send_email(to: &str, subject: &str, body: &str) ->
    Result<()>`
  - Best-effort: callers must not roll back on failure
  - The interface abstracts the email delivery mechanism

- [ ] 6. Define `NotificationCoalescer` interface (port) --
       `design.md#Components`, `requirements.md#US-1`
  - Method: `async fn should_send(result_id: i64, new_status: &str,
    user_id: i64) -> bool`
  - Returns `true` if the notification should be sent (first occurrence within
    the window), `false` if it should be suppressed (duplicate)
  - Uses a TTL-based key; implementations must be non-blocking

- [ ] 7. Implement `NotificationService` --
       `requirements.md#US-1` through `US-4`, `design.md#Sequence`
  - `on_result_status_changed(event)`: guards (no-change, invalid recipient,
    self-notification, coalescing), creates in-app notification, sends email
    (best-effort)
  - `list_notifications(user_id, query)`: paginated, filtered, scoped to user
  - `mark_as_read(notification_id, user_id)`: ownership check, idempotent
    update
  - `mark_all_as_read(user_id)`: bulk update, idempotent
  - Look up user email from UserRepository for email sending
  - Log warnings for skipped notifications (invalid recipient, coalescing)

- [ ] 8. Define command/query/response DTOs --
       `design.md#API Contract`
  - `ListNotificationsQuery` (page, limit, is_read?, type?)
  - `NotificationResponse` (id, type, title, body, is_read,
    related_object_type, related_object_id, created_at)
  - `MarkAllReadResponse` (count)
  - `PaginationMeta` (total, page, limit) -- reuse from existing

- [ ] 9. Write unit tests for `NotificationService` --
       `requirements.md#US-1` through `US-4`
  - Test `on_result_status_changed`: happy path creates notification + sends
    email
  - Test `on_result_status_changed`: no-change guard (old_status == new_status)
  - Test `on_result_status_changed`: invalid recipient (NULL, deleted,
    INACTIVE) skips notification
  - Test `on_result_status_changed`: self-notification suppressed (report_to
    == updater)
  - Test `on_result_status_changed`: coalescing suppresses duplicate
  - Test `on_result_status_changed`: email failure does not fail notification
    creation
  - Test `list_notifications`: pagination, is_read filter, type filter
  - Test `mark_as_read`: happy path, idempotent on already-read, cross-user
    forbidden
  - Test `mark_all_as_read`: happy path, zero unread returns count=0
  - Mock all three interfaces (NotificationRepository, EmailSender,
    NotificationCoalescer)

---

## Layer 3 -- Adapters (HTTP)

- [ ] 10. Implement `NotificationHandler` --
        `design.md#API Contract`, `design.md#Components`
  - Three handler methods: `list`, `mark_read`, `mark_all_read`
  - Deserialize query params into DTOs
  - Call `NotificationService` methods
  - Serialize responses with proper status codes (200, 403, 404, 422)

- [ ] 11. Register notification routes in HTTP router --
        `design.md#Components`
  - `GET    /api/v1/notifications`             -> `list`
  - `PATCH  /api/v1/notifications/{id}/read`   -> `mark_read`
  - `POST   /api/v1/notifications/read-all`    -> `mark_all_read`
  - All routes require session auth middleware
  - Route order: static (`/read-all`) before dynamic (`/{id}/read`)

- [ ] 12. Wire notification dispatch into result update flow --
        `requirements.md#US-1`, `design.md#Sequence`
  - After a test case result status change commits successfully, emit the
    `ResultStatusChanged` event
  - Dispatch event to `NotificationService::on_result_status_changed` via a
    post-commit hook or event bus
  - Ensure the event is only dispatched if the transaction committed (not on
    rollback)
  - The notification dispatch is non-blocking (fire and forget); if it fails,
    the result update is already committed

- [ ] 13. Write integration tests for notification HTTP handlers --
        `design.md#API Contract`
  - Test `GET /api/v1/notifications`: pagination, is_read filter, type filter,
    empty list
  - Test `PATCH /api/v1/notifications/{id}/read`: mark unread, idempotent on
    already-read
  - Test `PATCH /api/v1/notifications/{id}/read`: cross-user returns 403
  - Test `PATCH /api/v1/notifications/{id}/read`: non-existent returns 404
  - Test `POST /api/v1/notifications/read-all`: bulk mark, returns count
  - Test `POST /api/v1/notifications/read-all`: no unread returns count=0
  - Test auth: all endpoints return 401 without session cookie
  - Test end-to-end: update a result, verify notification created for
    report_to user

---

## Layer 4 -- Infrastructure

- [ ] 14. Create `NOTIFICATIONS` database migration --
        `design.md#Data Model`
  - Table definition with all columns, PK, FK (user_id -> users(id) ON DELETE
    CASCADE), CHECK constraint on type
  - Index: `idx_notifications_user_unread` -- partial index on (user_id,
    created_at DESC) WHERE is_read = FALSE
  - Index: `idx_notifications_user_created` -- index on (user_id, created_at
    DESC)
  - Index: `idx_notifications_user_unread_all` -- partial index on (user_id)
    WHERE is_read = FALSE (for mark-all-read bulk updates)
  - Rollback migration: `DROP TABLE IF EXISTS notifications`

- [ ] 15. Implement `SqlNotificationRepository` --
        `design.md#Components`, `design.md#Data Model`
  - All methods from `NotificationRepository` interface
  - `find_by_user_id`: parameterized query with optional is_read and type
    filters, LIMIT/OFFSET pagination
  - `mark_as_read`: `UPDATE notifications SET is_read = TRUE WHERE id = $1`
    (idempotent -- no WHERE is_read = FALSE clause)
  - `mark_all_as_read`: `UPDATE notifications SET is_read = TRUE WHERE
    user_id = $1 AND is_read = FALSE`; return count of affected rows
  - Write unit tests with a test transaction

- [ ] 16. Implement `SmtpEmailSender` --
        `design.md#Components`, `design.md#Configuration`
  - Use a Rust SMTP crate (e.g., `lettre`)
  - Configure connection from env vars: SMTP_HOST, SMTP_PORT, SMTP_USERNAME,
    SMTP_PASSWORD, SMTP_USE_TLS
  - `send_email(to, subject, body)`: build and send email with plain-text body
  - Set `From` address from SMTP_FROM_ADDRESS and SMTP_FROM_NAME config
  - Retry on transient failures up to EMAIL_MAX_RETRIES times with
    EMAIL_RETRY_DELAY_MS delay
  - Log ERROR on final failure; do not panic
  - Sanitize subject and body against header injection (strip CR and LF
    characters)

- [ ] 17. Implement `RedisNotificationCoalescer` --
        `design.md#Components`, `design.md#Configuration`
  - `should_send(result_id, new_status, user_id)`:
    - Construct Redis key:
      `coalesce:result:{result_id}:{new_status}:{user_id}`
    - Execute `SET key "1" EX {coalesce_seconds} NX`
    - If `NX` succeeds (key was not set), return `true`
    - If `NX` fails (key already exists), return `false`
  - On Redis connection failure: log WARN and return `true` (always send --
    coalescing is an optimization, not a correctness requirement)
  - TTL from `NOTIFICATION_COALESCE_SECONDS` env var (default 60)

---

## Verification & Cleanup

- [ ] 18. End-to-end verification --
        `requirements.md#US-1` through `US-4`
  - Update a test case result status; verify notification created for
    report_to user with correct title, body, related_object fields
  - Verify email is sent to report_to user's email address
  - Verify self-notification is suppressed
  - Verify notification is NOT created when old_status == new_status
  - Verify notification is NOT created for deleted/inactive report_to users
  - Verify coalescing: rapid updates to same result produce only one
    notification
  - Verify list, mark-read, mark-all-read endpoints work correctly
  - Verify cross-user access is blocked on mark-read

- [ ] 19. Update `specs/README.md` -- `specs/README.md`
  - Mark `notifications-email` as having completed specs (requirements.md,
    design.md, tasks.md)

---

## Security & Hardening

- [ ] 20. Implement email content sanitization --
        `requirements.md#security-considerations`
  - Strip CR (`\r`) and LF (`\n`) characters from ALL user-supplied fields used in
    email construction: test case summary, test run summary, and username, not just
    the subject line. This prevents SMTP header injection via any user-originated
    data path.
  - Escape HTML entities in email body if HTML format is used
  - Log sanitization events at DEBUG level for audit

- [ ] 21. Implement cross-user ownership checks --
        `requirements.md#US-3`, `design.md#Sequence`
  - `mark_as_read`: verify `notification.user_id == current_user_id`
  - Return `403 Forbidden` with generic message on mismatch
  - Integration test: attempt to mark another user's notification as read
