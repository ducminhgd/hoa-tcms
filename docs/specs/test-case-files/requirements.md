# Feature: Test Case Files

## Overview

Test Case Files are file attachments associated with a test case. Each file belongs to a
single test case and is stored on the server filesystem (or object storage). Files are scoped
to the project via the parent test case and follow the same authorization model as test cases:
Contributors can upload files to their own test cases and delete only the files they
themselves uploaded. Owners and Editors can manage all files on all test cases in the project.

Files are soft-deleted independently of their parent test case. Deleting a test case does not
cascade-delete its files; files remain in storage and can be cleaned up separately.

---

## User Stories

### US-1: Upload File to Test Case

As a project member with the `test_case_file:upload` system permission and a project role of
Contributor, Editor, or Owner, I want to upload a file attachment to a test case, so that I
can attach screenshots, logs, or supporting documents to my test scenarios.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_case_file:upload` permission (or a System Admin) sends a
  `POST /api/v1/projects/{projectId}/test-cases/{id}/files` request with a multipart file
  upload, THE SYSTEM SHALL validate the test case exists, is not soft-deleted, and belongs to
  the specified project, validate the file size does not exceed the configured maximum
  (default 10 MB), validate the file's MIME type is in the configured allowlist, generate a
  unique storage path, persist the file to the storage backend, insert a row into
  `TEST_CASE_FILES` with `file_name`, `file_path` (storage location), `file_size` (bytes),
  `mime_type`, and `uploaded_by` set to the authenticated user's ID, and return
  `201 Created` with the file metadata and a `Location` header.
- IF the user does not hold the `test_case_file:upload` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden` (generic message).
- IF the user holds `test_case_file:upload` but is a Viewer of the project AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden` (same generic message).
- IF the user holds `test_case_file:upload` and is a Contributor: THE SYSTEM SHALL verify
  that the test case's `created_by` matches the authenticated user's ID. If it does not
  match, THE SYSTEM SHALL return `403 Forbidden` with a distinct message indicating that
  Contributors can only upload files to their own test cases.
- IF the user holds `test_case_file:upload` and is an Owner or Editor: THE SYSTEM SHALL allow
  uploading files to any test case in the project regardless of `created_by`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the test case does not exist, is soft-deleted, or belongs to a different project, THE
  SYSTEM SHALL return `404 Not Found`.
- IF the uploaded file exceeds the configured maximum size (default 10 MB), THE SYSTEM SHALL
  return `413 Content Too Large` with error code `FILE_TOO_LARGE` and a message indicating
  the maximum allowed size.
- IF the uploaded file has no filename or the filename is empty after trimming, THE SYSTEM
  SHALL return `422 Unprocessable Entity` with error code `VALIDATION_ERROR` and a message
  indicating the filename is required.
- IF the file's MIME type is not in the configured allowlist, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with error code `FILE_TYPE_NOT_ALLOWED` and a message listing
  the allowed MIME types.
- IF the request body contains no file part or the file part is empty, THE SYSTEM SHALL
  return `422 Unprocessable Entity` with error code `VALIDATION_ERROR`.

### US-2: List Files Attached to a Test Case

As a project member, I want to view a list of all non-deleted files attached to a test case,
so that I can see what attachments exist before downloading or deleting them.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_case_file:read` permission (or a System Admin) sends
  `GET /api/v1/projects/{projectId}/test-cases/{id}/files`, THE SYSTEM SHALL return an
  unpaginated list of all non-deleted file metadata records for the given test case, ordered
  by `created_at` ascending (oldest first).
- IF the user does not hold the `test_case_file:read` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_case_file:read` but is not a member of the project AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the test case does not exist, is soft-deleted, or belongs to a different project, THE
  SYSTEM SHALL return `404 Not Found`.
- THE SYSTEM SHALL exclude soft-deleted files from the list.
- Each item in the response SHALL include `id`, `file_name`, `file_size`, `mime_type`,
  `uploaded_by`, `created_at`, and `updated_at`. The `file_path` SHALL NOT be exposed.
- THE SYSTEM SHALL support an optional `search` query parameter (max 255 characters) for
  case-insensitive substring match on `file_name`. Leading/trailing whitespace is trimmed;
  an all-whitespace search is treated as "no filter". The search value escape order is:
  `\` -> `\\`, then `%` -> `\%`, then `_` -> `\_`.
- THE SYSTEM SHALL support an optional `sort` query parameter with values `file_name`,
  `-file_name` (descending), `file_size`, `-file_size` (descending), `created_at`,
  `-created_at` (descending, default). Invalid sort values return `422`.
- IF the test case has no files (or all files are soft-deleted), THE SYSTEM SHALL return an
  empty array with `200 OK`.

### US-3: Download a File

As a project member, I want to download a file attached to a test case, so that I can view
its contents locally.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_case_file:read` permission (or a System Admin) sends
  `GET /api/v1/projects/{projectId}/test-cases/{id}/files/{fileId}`, THE SYSTEM SHALL read
  the file from the storage backend, set the `Content-Type` header to the file's `mime_type`,
  set the `Content-Disposition` header to `attachment; filename="{original_file_name}"`, set
  the `Content-Length` header to `file_size`, and stream the file contents in the response
  body.
- IF the user does not hold the `test_case_file:read` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_case_file:read` but is not a member of the project AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the test case does not exist, is soft-deleted, or belongs to a different project, THE
  SYSTEM SHALL return `404 Not Found`.
- IF the file record does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the file exists in the database but the physical file is missing from the storage
  backend, THE SYSTEM SHALL log an ERROR, return `404 Not Found`, and NOT expose the storage
  path in the response.
- THE SYSTEM SHALL NOT return the storage path (`file_path`) to the client in any
  circumstance.

### US-4: Delete a File (Soft-delete)

As a project member, I want to soft-delete a file attachment from a test case, so that it is
hidden from the file list while its data is preserved for potential recovery. As a Contributor
I can only delete files I uploaded. As an Owner or Editor I can delete any file.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_case_file:delete` permission (or a System Admin) sends
  `DELETE /api/v1/projects/{projectId}/test-cases/{id}/files/{fileId}`, THE SYSTEM SHALL set
  `deleted_at = NOW()` and `deleted_by = <current_user_id>` on the file record and return
  `204 No Content`. The physical file on the storage backend SHALL NOT be removed (it is
  preserved for potential recovery).
- IF the user does not hold the `test_case_file:delete` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_case_file:delete` but is a Viewer of the project AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_case_file:delete` and is a Contributor: THE SYSTEM SHALL verify
  that the file's `uploaded_by` matches the authenticated user's ID. If it does not match,
  THE SYSTEM SHALL return `403 Forbidden` with a distinct message indicating that
  Contributors can only delete files they uploaded.
- IF the user holds `test_case_file:delete` and is an Owner or Editor: THE SYSTEM SHALL allow
  deleting any file regardless of `uploaded_by`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the test case does not exist, is soft-deleted, or belongs to a different project, THE
  SYSTEM SHALL return `404 Not Found`.
- IF the file record does not exist or is already soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- THE SYSTEM SHALL NOT check for referential integrity constraints before soft-deleting (files
  are leaf entities -- no other tables reference `TEST_CASE_FILES`).
- THE SYSTEM SHALL NOT hard-delete any record or physical file.
- After soft-delete, the file is excluded from the list and download endpoints.

### US-5: File Size and Type Validation

As a system administrator, I want file size and MIME type limits to be configurable so that
I can enforce security policies without code changes. As a user, I want clear error messages
when my upload is rejected so that I know exactly what to fix.

**Acceptance Criteria (EARS)**

- THE SYSTEM SHALL enforce a maximum file size limit defined in application configuration
  (default 10 MB). The limit applies at the application layer (before the file is written to
  disk) and as a middleware-level request body size limit as defence-in-depth.
- THE SYSTEM SHALL enforce a MIME type allowlist defined in application configuration.
  The allowlist SHALL include common document, image, and archive types by default:
  `application/pdf`, `image/png`, `image/jpeg`, `image/gif`, `image/webp`, `text/plain`,
  `text/csv`, `application/json`, `application/zip`, `application/x-tar`,
  `application/gzip`, `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`,
  `application/vnd.openxmlformats-officedocument.wordprocessingml.document`,
  `application/vnd.ms-excel`, `application/vnd.ms-powerpoint`.
- WHEN a file is uploaded, THE SYSTEM SHALL detect the MIME type from the file content
  (magic bytes / content sniffing), not from the client-supplied `Content-Type` header. If
  the detected MIME type does not match any entry in the allowlist, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with error code `FILE_TYPE_NOT_ALLOWED`.
- IF the application configuration changes, THE SYSTEM SHALL enforce the new limits on the
  next request without requiring a restart (hot-reload of config or read-on-request).
- THE SYSTEM SHALL enforce a maximum filename length of 255 characters (as stored in the
  `file_name` column). Longer filenames return `422 Unprocessable Entity`.

---

## Security Considerations

### Authentication
All endpoints require a valid authenticated session. Requests without a valid session cookie
return `401 Unauthorized` with error code `NOT_AUTHENTICATED`. This is enforced by
`AuthMiddleware` before any handler logic executes.

### Authorization Consistency
The system permission check is the first authorization gate; the project membership and role
check is the second gate. For Contributors, two different ownership checks apply:
- **Upload**: Contributor must own the test case (`test_case.created_by == current_user_id`).
  This prevents Contributors from attaching files to test cases created by others.
- **Delete**: Contributor must own the file (`file.uploaded_by == current_user_id`). This
  prevents Contributors from deleting files uploaded by others, even on their own test cases.
Owners and Editors bypass all ownership checks. System Admin bypasses all gates.

A `403 Forbidden` response must use a generic message without revealing which gate was
triggered, except for the two Contributor ownership checks which return distinct messages
("Contributors can only upload files to their own test cases" and "Contributors can only
delete files they uploaded") because the user has already passed the project membership gate.

### Input Validation
- Filenames must be sanitized to prevent path traversal attacks. Characters that could be
  used for directory traversal (`../`, `..\\`) must be rejected or stripped.
- The `file_name` stored in the database is the original client-supplied filename (sanitized).
  The `file_path` is a server-generated path that does not incorporate user input beyond the
  project ID and test case ID (both validated integers).
- File content is never executed or interpreted by the server. MIME type detection uses
  content inspection, not the client-supplied Content-Type.
- Uploaded files must not be stored in a web-accessible directory (no direct URL access).

### File Storage Security
- The storage backend directory must be outside the web server's document root.
- File paths are generated server-side using a UUID and the sanitized original extension,
  organized in a directory hierarchy: `{base_upload_dir}/{project_id}/{test_case_id}/{uuid}.{ext}`.
- The `file_path` column is never returned in API responses.
- Download is only permitted through the API endpoint, which enforces authentication and
  authorization.

### Rate Limiting
All endpoints apply rate limiting configured at the infrastructure/middleware layer:

| Endpoint | Rate Limit | Window |
|----------|-----------|--------|
| `POST .../files` (upload) | 30 requests | per minute |
| `GET .../files` (list) | 60 requests | per minute |
| `GET .../files/{fileId}` (download) | 60 requests | per minute |
| `DELETE .../files/{fileId}` (delete) | 30 requests | per minute |

Rate limit state is communicated via `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and
`X-RateLimit-Reset` response headers. Exceeding the limit returns `429 Too Many Requests`
with a `Retry-After` header.

### File Content Safety
- MIME type is validated by content inspection (magic bytes), not by the client-supplied
  Content-Type header, to prevent MIME type spoofing.
- The `Content-Disposition: attachment` header on downloads forces the browser to download
  rather than render the file inline, preventing XSS via uploaded HTML/SVG files.
- File size is validated at the application layer before the file is written to disk,
  preventing disk exhaustion attacks. A middleware-level body size limit provides
  defence-in-depth.

---

## Out of Scope

- **Multiple file upload in a single request** (batch upload -- Phase 2)
- **File preview / thumbnails** (inline preview for images, PDFs -- separate feature)
- **File versioning** (replacing an existing file with a new version -- separate feature)
- **Physical file deletion** (purging soft-deleted files from disk -- deferred to a future
  data retention/purge feature)
- **File restore** (clearing `deleted_at` / `deleted_by` -- deferred to a future feature)
- **File move / re-associate** (moving a file from one test case to another)
- **Bulk download** (zip download of all files for a test case, test run, or project)
- **Direct file links / signed URLs** (time-limited download URLs -- deferred to Phase 2)
- **Object storage backends** (S3, GCS, Azure Blob -- initial implementation uses local
  filesystem; the storage adapter interface supports future backends)
- **UI views** (file list, upload button, download link, delete confirmation -- covered in
  `ui-file-upload` spec)

---

## Dependencies

- **IAM Auth** -- Session-based authentication; all endpoints require an authenticated
  session via `AuthMiddleware`.
- **IAM Permissions** -- The permission codes `test_case_file:upload`, `test_case_file:read`,
  `test_case_file:delete` must be seeded in the `PERMISSIONS` table and assignable to roles.
- **IAM Users** -- `users.id` FK reference for `uploaded_by` and `deleted_by` audit columns.
- **Project CRUD** -- The `PROJECTS` table must exist; project existence is validated on
  every request.
- **Project Members** -- Project membership and roles (Owner, Editor, Contributor, Viewer)
  are used for authorization scope checks. The Contributor role has ownership-based
  restrictions for upload (must own the test case) and delete (must own the file).
- **Test Case CRUD** -- The `TEST_CASES` table must exist; files reference
  `test_cases(id)` via `test_case_id` FK. Test case existence, project scope, and
  soft-delete status must be validated before any file operation. The `created_by` field on
  test cases is checked for the Contributor upload ownership restriction.
- **Auth RBAC** -- System permission checks for `test_case_file:*` codes.
- **Auth Project Scope** -- Project membership scope check on all endpoints.
- **UI File Upload** -- The file upload UI component follows the project-wide file upload
  pattern (size/type restrictions from config, progress indication, error display).
