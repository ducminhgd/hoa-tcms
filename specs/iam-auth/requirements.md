# Feature: IAM Auth

## Overview

Provide login and logout authentication for the HOA TCMS. Authenticated users receive a
session-based secure HTTP-only cookie. The system uses PBKDF2 password hashing and rejects
INACTIVE users without revealing account status.

## User Stories

### US-01: Login

As a registered user, I want to log in with my username or email and my password, so that
I can access the system.

**Acceptance Criteria (EARS)**

- WHEN I submit a `POST /api/v1/auth/login` request with a valid `username` or `email`
  and the matching `password`, THE SYSTEM SHALL create a session in Redis, set a secure
  HTTP-only session cookie on the response, and return `200 OK`.
- IF I submit a `username`/`email` that does not match any user record, THE SYSTEM SHALL
  return `401 Unauthorized` with a generic error message that does not distinguish between
  "user not found" and "wrong password".
- IF I submit a `username`/`email` that belongs to an INACTIVE user, THE SYSTEM SHALL
  return `401 Unauthorized` with the same generic error message as invalid credentials.
- IF I submit a `password` shorter than 8 characters, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with a validation error.
- IF I submit both `username` and `email` in the same request, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with a validation error (exactly one of the two is required).
- IF I submit neither `username` nor `email`, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with a validation error.
- WHEN I use an email to log in, THE SYSTEM SHALL perform a case-insensitive lookup
  of the `email` column.
- WHEN I use a username to log in, THE SYSTEM SHALL perform a case-insensitive lookup
  of the `username` column.
- IF I submit valid credentials but the session store (Redis) is unavailable, THE SYSTEM
  SHALL return `503 Service Unavailable` and log the failure.

### US-02: Logout

As an authenticated user, I want to log out, so that my session is terminated and
subsequent requests are rejected.

**Acceptance Criteria (EARS)**

- WHEN I submit a `POST /api/v1/auth/logout` request with a valid session cookie,
  THE SYSTEM SHALL delete the session from Redis, clear the session cookie on the
  response, and return `200 OK`.
- IF I submit `POST /api/v1/auth/logout` without a session cookie or with an invalid
  or expired session, THE SYSTEM SHALL return `401 Unauthorized`.
- WHILE the session cookie has been cleared, any subsequent request with the same
  cookie value SHALL be treated as unauthenticated.

### US-03: Session Verification

As a system component, I want to verify that every request to a protected endpoint
carries a valid session, so that only authenticated users can access the system.

**Acceptance Criteria (EARS)**

- WHEN a request arrives at any protected endpoint, THE SYSTEM SHALL read the session
  cookie, look up the session ID in Redis, and verify it has not expired.
- IF the session cookie is missing, malformed, expired, or not found in Redis,
  THE SYSTEM SHALL return `401 Unauthorized`.
- IF the session is valid, THE SYSTEM SHALL extract the authenticated user's ID from
  the session payload and attach it to the request context for downstream use by
  authorization middleware and business logic.
- WHILE a user's status has been changed to INACTIVE, any subsequent request carrying
  an existing valid session for that user SHALL be rejected with `403 Forbidden`.

### US-04: Password Hashing

As a security-conscious system, I want to hash passwords using PBKDF2 before storage,
so that credentials are protected even if the database is compromised.

**Acceptance Criteria (EARS)**

- WHEN a password is stored or updated in the database, THE SYSTEM SHALL hash it using
  PBKDF2 with the format string `pbkdf2$<algorithm>$<salt>$<iterations>$<hash>`.
- WHEN a login request is processed, THE SYSTEM SHALL compare the provided password
  against the stored hash using a constant-time comparison function.
- IF the configured PBKDF2 iteration count is increased after a user's password was
  last hashed, THE SYSTEM SHALL re-hash and re-store the password on the user's next
  successful login (transparent to the user).

### US-05: Session Configuration

As a system administrator, I want configurable session parameters, so that I can
control session lifetime and cookie security settings per deployment environment.

**Acceptance Criteria (EARS)**

- WHEN the application starts, THE SYSTEM SHALL read session configuration (TTL, cookie
  name, cookie path, `Secure` flag, `SameSite` policy) from environment variables with
  sensible defaults.
- WHEN a session cookie is created, THE SYSTEM SHALL set it with the following
  attributes: `HttpOnly`, `Secure` flag controlled by the `SESSION_SECURE` environment
  variable (default `true`), `SameSite=Lax`, `Path` from configuration, and `Max-Age`
  equal to the configured TTL.
- WHEN a session expires (TTL elapsed), any request with that session cookie SHALL be
  treated as unauthenticated and return `401 Unauthorized`.

## Out of Scope

- User self-registration (accounts are created by System Admin only).
- Password reset or "forgot password" flow.
- Account lockout after failed login attempts (per FR-02).
- Multi-factor authentication (MFA / 2FA).
- OAuth 2.0, SSO, LDAP, or any external identity provider integration.
- API tokens or personal access tokens for programmatic access.
- "Remember me" or persistent long-lived sessions.
- Sliding session expiration (TTL is fixed and not refreshed on activity).
- Session list or session management UI (admin cannot view/revoke active sessions).
- Audit logging of login/logout events (deferred to a cross-cutting audit feature).
- Session fingerprinting (IP, User-Agent) — sessions are validated by session ID only.
- Rate limiting on login attempts (deferred to a cross-cutting rate-limiting feature).

## Dependencies

- **USERS table** in PostgreSQL — must exist with columns: `id`, `username`, `email`,
  `password_hash`, `fullname`, `status`, `deleted_at` (from `iam-users` feature).
- **Redis** — session store. Must be available and configured.
- **Configuration loader** — environment variables for session TTL, cookie name, etc.
- **Auth middleware** — shared middleware component that other features depend on for
  request authentication.
