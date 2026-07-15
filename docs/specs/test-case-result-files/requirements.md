# Feature: Test Case Result Files

## Overview

Test Case Result Files enables attaching files (screenshots, logs, PDFs, etc.) to Test Case
Results within a Test Execution. Each file belongs to exactly one Test Case Result.
Operations are authorized under the same permission (`test_execution:update`) as updating the
parent result, following the rule that permission on a Test Execution extends to its child
results (FR-44).

Files are stored on the filesystem under a configurable upload directory, with metadata
tracked in the `TEST_RESULT_FILES` database table. A file's lifecycle is independent of the
parent result: soft-deleting a result does not cascade-delete its files, and vice versa.

---

## User Stories

### US-1: List Files on a Test Case Result

As a user with `test_execution:update` permission and the appropriate project membership,
I want to list all files attached to a Test Case Result, so that I can see what evidence
(screenshots, logs, documents) has been uploaded for a given result.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_execution:update` permission (or a System Admin) sends
  `GET /api/v1/test-case-results/{resultId}/files`, THE SYSTEM SHALL return a flat array
  of all non-deleted files attached to that Test Case Result, each containing `id`,
  `file_name`, `file_size`, `mime_type`, `uploaded_by`, and `created_at`.
- IF the user does not hold the `test_execution:update` system permission AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden` (generic message).
- IF the user holds `test_execution:update` but is not a member of the project that
  contains the parent Test Execution AND is not a System Admin, THE SYSTEM SHALL return
  `403 Forbidden` (same generic message).
- IF the Test Case Result does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the parent Test Execution does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- THE SYSTEM SHALL NOT paginate this endpoint -- result file collections are expected to
  be small (typically under 20 files per result).
- THE SYSTEM SHALL exclude soft-deleted files from the list.
- IF there are no files attached, THE SYSTEM SHALL return an empty array with `200 OK`.

### US-2: Upload File to Test Case Result

As a user with `test_execution:update` permission, I want to upload a file and attach it
to a specific Test Case Result, so that I can provide supporting evidence (screenshots,
error logs, test data) for the result.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_execution:update` permission (or a System Admin) sends
  `POST /api/v1/test-case-results/{resultId}/files` with a multipart form-data body
  containing a single file, THE SYSTEM SHALL validate the file's MIME type and size against
  the configured limits, generate a unique storage filename, save the file to the configured
  upload directory, insert a row into `TEST_RESULT_FILES` with the metadata (original file
  name, storage path, file size in bytes, MIME type, and `uploaded_by` set to the
  authenticated user), and return `201 Created` with the file metadata and a `Location`
  header pointing to the new file resource.
- IF the user does not hold the `test_execution:update` system permission AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden` (generic message).
- IF the user holds `test_execution:update` but is not a member of the project that
  contains the parent Test Execution AND is not a System Admin, THE SYSTEM SHALL return
  `403 Forbidden` (same generic message).
- IF the Test Case Result does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the parent Test Execution does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the uploaded file exceeds the configured maximum size (`MAX_FILE_SIZE_BYTES`, default
  10 MB), THE SYSTEM SHALL return `422 Unprocessable Entity` with error code
  `FILE_TOO_LARGE` and a message indicating the maximum allowed size.
- IF the uploaded file's MIME type is not in the configured allowed types
  (`ALLOWED_MIME_TYPES`, default: PNG, JPEG, PDF, TXT, ZIP, CSV, JSON, XML),
  THE SYSTEM SHALL return `422 Unprocessable Entity` with error code `FILE_TYPE_NOT_ALLOWED`
  and a message listing the allowed types.
- IF no file is provided in the multipart form-data, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with error code `VALIDATION_ERROR` and a message indicating
  that a file is required.
- IF multiple files are provided in a single request, THE SYSTEM SHALL process only the
  first file and ignore the rest (single-file upload per request).
- IF the file name contains path traversal characters (e.g. `../`, `..\\`), THE SYSTEM
  SHALL sanitize the stored `file_name` by stripping directory components -- only the
  base file name is stored. The original name (without directory components) is preserved
  for display purposes.
- THE SYSTEM SHALL generate the storage file name as a UUID v4 with the original file
  extension appended (e.g., `<uuid>.png`), preventing file name collisions and path
  traversal attacks on the filesystem.
- IF disk write fails (e.g. disk full, permission denied on upload directory),
  THE SYSTEM SHALL return `500 Internal Server Error`, log the error at ERROR level,
  and NOT insert a database record (the file record is only created after a successful
  file write).
- THE SYSTEM SHALL set both `created_by` and `uploaded_by` to the authenticated user's
  ID on creation.

### US-3: Download File

As a user with permission on the Test Case Result, I want to download an attached file,
so that I can view screenshots, open logs, or inspect test evidence.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_execution:update` permission (or a System Admin) sends
  `GET /api/v1/files/{fileId}`, THE SYSTEM SHALL look up the file metadata in
  `TEST_RESULT_FILES`, resolve the Test Case Result and its parent Test Execution to
  verify project membership, read the file from the filesystem, set appropriate
  `Content-Type` and `Content-Disposition: attachment; filename="<original_name>"` headers,
  and stream the file content to the client.
- IF the user does not hold `test_execution:update` system permission AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden` (generic message).
- IF the user holds `test_execution:update` but is not a member of the project that
  contains the parent Test Execution AND is not a System Admin, THE SYSTEM SHALL return
  `403 Forbidden` (same generic message).
- IF the file record does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the file is not found on disk (e.g., deleted manually, storage corruption),
  THE SYSTEM SHALL return `404 Not Found`, log the error at WARN level, and NOT reveal
  the filesystem path in the response.
- THE SYSTEM SHALL set `Content-Type` to the stored `mime_type` from the database.
- THE SYSTEM SHALL set `Content-Disposition` header to `attachment; filename="<file_name>"`
  using the original file name (properly quoted for HTTP header encoding).
- THE SYSTEM SHALL NOT require that the parent Test Case Result or Test Execution is
  non-deleted for download -- files of soft-deleted results can still be downloaded
  (the file's own soft-delete status is the only deletion gate for download access).
- THE SYSTEM SHALL stream the file content (not buffer the entire file in memory for large
  files).

### US-4: Delete File (Soft-delete)

As a user with `test_execution:update` permission, I want to soft-delete an attached file
from a Test Case Result, so that I can remove incorrect or outdated attachments while
preserving the record for audit purposes.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_execution:update` permission (or a System Admin) sends
  `DELETE /api/v1/files/{fileId}`, THE SYSTEM SHALL set `deleted_at = NOW()` and
  `deleted_by = <current_user_id>` on the file record and return `204 No Content`.
- IF the user does not hold the `test_execution:update` system permission AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden` (generic message).
- IF the user holds `test_execution:update` but is not a member of the project that
  contains the parent Test Execution AND is not a System Admin, THE SYSTEM SHALL return
  `403 Forbidden` (same generic message).
- IF the file record does not exist or is already soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- THE SYSTEM SHALL NOT delete the physical file from disk on soft-delete (the file remains
  on disk for audit trail and potential future restore).
- THE SYSTEM SHALL NOT check whether the parent Test Case Result or Test Execution is
  soft-deleted -- a file can be deleted regardless of the parent's deletion status.
- THE SYSTEM SHALL NOT hard-delete any file record.

---

## Out of Scope

- **Bulk file upload** (multiple files in one request -- Phase 1 supports single-file upload
  only)
- **File restore** (clearing `deleted_at` / `deleted_by` -- deferred to a future phase;
  data is preserved but no restore endpoint exists in Phase 1)
- **Hard delete** (not implemented for any entity in Phase 1)
- **Thumbnail generation** for image files (deferred to a future phase)
- **Inline preview** of file content (download only in Phase 1; inline rendering deferred)
- **File versioning** (re-uploading with the same name creates a new file record; no
  version history)
- **File move/copy** between Test Case Results
- **File sharing** independent of parent result (file access inherits from the parent
  Test Case Result's authorization)

---

## Dependencies

- **IAM Auth** -- Session-based authentication; all endpoints require an authenticated
  session via `AuthMiddleware`.
- **IAM Permissions** -- The permission code `test_execution:update` must be seeded in
  the `PERMISSIONS` table. File operations reuse this existing permission; no new
  permission codes are introduced.
- **IAM Users** -- `users.id` FK reference for `uploaded_by`, `created_by`, `updated_by`,
  `deleted_by` audit columns.
- **Test Execution CRUD** -- Test Case Results are children of Test Executions. The
  `TEST_CASE_RESULTS` table must exist with `execution_id` FK to `TEST_EXECUTIONS`.
  The project scope is resolved by traversing `TEST_CASE_RESULTS -> TEST_EXECUTIONS ->
  TEST_RUNS -> TEST_PLANS -> PROJECTS` (or equivalent path through the execution's project).
- **Test Case Result Update** -- File operations share the same authorization model
  (`test_execution:update`) as result updates per FR-44.
- **Auth RBAC** -- System permission check for `test_execution:update` code.
- **Auth Project Scope** -- Project membership scope check resolved through the execution's
  project chain.
- **UI File Upload** -- Uses the shared file upload component with configurable size/type
  restrictions.

---

## Security Considerations

### Authentication

All endpoints require a valid authenticated session. Requests without a valid session cookie
return `401 Unauthorized` with error code `NOT_AUTHENTICATED`. This is enforced by
`AuthMiddleware` before any handler logic executes.

### Authorization

File operations are authorized under `test_execution:update`, the same permission used for
updating the parent Test Case Result (FR-44: permission on a Test Execution extends to its
child results). The authorization check follows the three-gate model:

1. System permission check (`test_execution:update`)
2. Project membership check (resolved from the file's parent Test Case Result, up through
   the Test Execution to its containing project)
3. System Admin bypasses all gates

A `403 Forbidden` response must use a generic message without revealing which gate was
triggered.

### File Name and Path Traversal

All user-supplied file names are sanitized at the boundary:
- Directory components in the original file name are stripped; only the base name is stored
  in `file_name` for display.
- The storage file name is always a server-generated UUID v4 with the original extension
  appended. No user-supplied data is used as the actual filesystem path.
- The upload directory is a dedicated, configurable path outside the web root. The server
  never serves files directly from this directory via URL mapping -- all file access goes
  through the download handler which enforces authorization.

### File Type and Size Validation

File type and size are validated **before** the file is written to disk:
1. Check the `Content-Length` header (if present) against `MAX_FILE_SIZE_BYTES` before
   reading the body.
2. Check the MIME type from the multipart part header against `ALLOWED_MIME_TYPES`.
3. After the file is fully received, verify the actual size on disk does not exceed the
   limit (defence in depth against `Content-Length` spoofing).

The MIME type check is based on the `Content-Type` header in the multipart part, not on
file extension or magic bytes. This provides a reasonable first line of defence; magic-byte
validation should be considered for a future security hardening iteration.

### Rate Limiting

| Endpoint | Rate Limit | Window |
|----------|-----------|--------|
| `GET .../results/{resultId}/files` (list) | 60 requests | per minute |
| `POST .../results/{resultId}/files` (upload) | 30 requests | per minute |
| `GET .../files/{fileId}` (download) | 60 requests | per minute |
| `DELETE .../files/{fileId}` (delete) | 30 requests | per minute |

Rate limit state is communicated via `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and
`X-RateLimit-Reset` response headers. Exceeding the limit returns `429 Too Many Requests`
with a `Retry-After` header.

### CSRF Protection

All state-changing endpoints (`POST`, `DELETE`) must be protected against Cross-Site
Request Forgery. Session cookies must carry `SameSite=Lax`. For `POST` endpoints with
`multipart/form-data`: CSRF token validation is required because `SameSite` cookies are
attached to simple cross-origin form submissions.

### Sensitive Data in Logs

File names and paths must not be logged at INFO level or above (they may contain
descriptive information). Log file operations using the `file_id` rather than the file
name or path. On error conditions, file paths may be logged at DEBUG level only.

---

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `UPLOAD_DIR` | `./uploads` | Filesystem directory for storing uploaded files. Must be writable by the application process. |
| `MAX_FILE_SIZE_BYTES` | `10485760` (10 MB) | Maximum allowed file size in bytes. |
| `ALLOWED_MIME_TYPES` | `image/png,image/jpeg,application/pdf,text/plain,application/zip,text/csv,application/json,application/xml` | Comma-separated list of allowed MIME types. |
