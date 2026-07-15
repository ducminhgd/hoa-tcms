# Feature: Email & In-App Notifications

## Overview

Notifications keep users informed of important events in the test case management
workflow. When a test case result status changes (e.g., from IN PROGRESS to PASS
or FAIL), the system notifies the user designated as `report_to` on the parent
test run. Notifications are delivered through two channels: email (via SMTP or an
external email service) and in-app (stored in the database and surfaced in the
UI). Users can view, mark as read, and bulk-mark their notifications.

---

## User Stories

### US-1: Receive Notification on Execution Result Change

As a user assigned as `report_to` on a test run, I want to be notified when a test
case result's status changes, so that I know when test outcomes have been updated
without having to poll the system.

**Acceptance Criteria (EARS)**

- WHEN a test case result's status field is updated to PASS, FAIL, WARNING, or
  IGNORE, THE SYSTEM SHALL create an in-app notification in the NOTIFICATIONS table
  addressed to the `report_to` user of the result's ancestor test run, with
  `type = 'TEST_CASE_RESULT_CHANGE'`, `related_object_type = 'test_case_result'`,
  and `related_object_id` set to the result's ID.
- THE SYSTEM SHALL generate the notification title as `"Result changed to {status}
  for TC-{test_case_id}: {test_case_summary}"` and the notification body as
  `"The test case result for '{test_case_summary}' in test run '{test_run_summary}'
  was changed from {old_status} to {new_status} by {updated_by_username}."`
- WHEN the notification is created, THE SYSTEM SHALL also send an email to the
  `report_to` user's email address with the same title as the subject and the same
  body as the email body, via the configured SMTP email service.
- IF the `report_to` user does not exist, is soft-deleted, or is INACTIVE, THE
  SYSTEM SHALL skip notification creation (both in-app and email) and log a warning.
- IF the `report_to` user is the same as the user who updated the result, THE SYSTEM
  SHALL skip notification creation (both in-app and email) -- users should not be
  notified of their own changes.
- IF the SMTP email service is unreachable or returns an error, THE SYSTEM SHALL
  still create the in-app notification and log the email delivery failure as ERROR.
  The in-app notification is the system of record; email is best-effort.
- IF the result update fails (transaction rolled back), THE SYSTEM SHALL NOT create
  any notification.
- WHEN the same result is updated multiple times within a short window (e.g., 60
  seconds) by the same user, THE SYSTEM SHALL coalesce updates: only the latest
  status change triggers a notification. This prevents notification spam from rapid
  consecutive edits.
- IF the old status and new status are the same (no actual change), THE SYSTEM SHALL
  NOT create a notification (this can happen when a PATCH request re-sends the
  current status).

### US-2: List Notifications

As an authenticated user, I want to view a paginated list of my notifications, so
that I can see recent updates and unread items in one place.

**Acceptance Criteria (EARS)**

- WHEN a user sends `GET /api/v1/notifications`, THE SYSTEM SHALL return a paginated
  list of notifications addressed to the authenticated user, ordered by `created_at`
  descending (newest first).
- IF the user is not authenticated, THE SYSTEM SHALL return `401 Unauthorized`.
- THE SYSTEM SHALL support pagination via query parameters `page` (default 1,
  minimum 1) and `limit` (default 25, minimum 1, maximum 100).
- THE SYSTEM SHALL include pagination metadata (`total`, `page`, `limit`) in the
  response.
- THE SYSTEM SHALL support an optional `is_read` query parameter (boolean `true` or
  `false`) that filters notifications by read status. Invalid values return `422`.
- THE SYSTEM SHALL support an optional `type` query parameter that filters
  notifications by type. Currently the only type is `TEST_CASE_RESULT_CHANGE`.
  Unknown type values return `422`.
- Each notification in the response SHALL include: `id`, `type`, `title`, `body`,
  `is_read`, `related_object_type`, `related_object_id`, and `created_at`.

### US-3: Mark Notification as Read

As an authenticated user, I want to mark a single notification as read, so that I
can acknowledge that I have seen it and keep my notification list tidy.

**Acceptance Criteria (EARS)**

- WHEN a user sends `PATCH /api/v1/notifications/{id}/read`, THE SYSTEM SHALL set
  `is_read = true` on the notification with the given ID and return `200 OK` with
  the updated notification representation.
- IF the user is not authenticated, THE SYSTEM SHALL return `401 Unauthorized`.
- IF the notification does not exist, THE SYSTEM SHALL return `404 Not Found`.
- IF the notification's `user_id` does not match the authenticated user's ID, THE
  SYSTEM SHALL return `403 Forbidden` (users cannot mark other users' notifications
  as read).
- IF the notification is already marked as read, THE SYSTEM SHALL return `200 OK`
  with the notification representation (idempotent -- no error for double-read).

### US-4: Mark All Notifications as Read

As an authenticated user, I want to mark all my unread notifications as read in a
single action, so that I can quickly clear my notification backlog.

**Acceptance Criteria (EARS)**

- WHEN a user sends `POST /api/v1/notifications/read-all`, THE SYSTEM SHALL set
  `is_read = true` on all unread notifications belonging to the authenticated user
  and return `200 OK` with `{ "data": { "count": <number of notifications marked
  as read> } }`.
- IF the user is not authenticated, THE SYSTEM SHALL return `401 Unauthorized`.
- IF the user has no unread notifications, THE SYSTEM SHALL return `200 OK` with
  `count = 0` (idempotent -- no error for zero affected rows).

---

## Security Considerations

### Authentication
All notification endpoints require a valid authenticated session. Requests without
a valid session cookie return `401 Unauthorized` with error code
`NOT_AUTHENTICATED`. This is enforced by `AuthMiddleware` before any handler logic
executes.

### Authorization
Users can only access their own notifications. The list endpoint automatically
scopes to the authenticated user's `user_id`. The mark-read endpoint verifies
`user_id` ownership before applying the update. Users cannot read, mark, or
otherwise interact with notifications addressed to other users.

### Email Content Sanitization
Email subject and body are constructed from user-supplied data (test case summary,
test run summary, usernames). These values must be sanitized against header
injection (carriage return and line feed characters stripped) and HTML injection
(if emails are sent as HTML).

### Rate Limiting (In-App Notification Endpoints)

| Endpoint | Rate Limit | Window |
|----------|-----------|--------|
| `GET /api/v1/notifications` (list) | 60 requests | per minute |
| `PATCH /api/v1/notifications/{id}/read` | 60 requests | per minute |
| `POST /api/v1/notifications/read-all` | 10 requests | per minute |

### Notification Coalescing
Duplicate suppression (60-second window) prevents a malicious or buggy client
from flooding a user with notifications by rapidly updating the same test case
result. The coalescing window is configurable via environment variable.

---

## Out of Scope

- **Push notifications** (browser push, mobile push -- Phase 1 is email + in-app
  only)
- **Notification preferences** (per-user opt-in/opt-out per notification type --
  deferred to a future phase)
- **Rich HTML email templates** (plain-text emails in Phase 1; HTML templates
  deferred)
- **Notification for events other than test case result changes** (e.g., project
  membership changes, sharing changes -- deferred to future features)
- **Notification delete** (notifications are retained indefinitely; a cleanup
  policy may be added in a future phase)
- **Notification search / full-text filtering beyond type and read status**
- **WebSocket real-time push** (polling-based in Phase 1; real-time deferred)

---

## Dependencies

- **IAM Auth** -- Session-based authentication; all endpoints require an
  authenticated session via `AuthMiddleware`.
- **IAM Users** -- `users.id` FK reference for `NOTIFICATIONS.user_id`. User email
  is read from the `USERS` table for email delivery.
- **Test Run CRUD** -- Test runs carry the `report_to` user ID; this field is read
  when determining the notification recipient.
- **Test Execution Management** -- Test case result status updates are the trigger
  event for notifications. The notification logic is triggered as a side effect of
  result status changes (domain event or service-layer hook).
- **SMTP / Email Service** -- An SMTP server or email service (e.g., SendGrid,
  Mailgun) must be configured and reachable for email delivery. Configuration is via
  environment variables.
