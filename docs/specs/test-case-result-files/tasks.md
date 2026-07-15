# Tasks: Test Case Result Files

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Implement domain layer (entity, value objects, exceptions) --
       `requirements.md#US-1` through `US-4`, `design.md#Components`

  **Entity `TestResultFile`:**
  - Fields: `id`, `test_case_result_id`, `file_name`, `file_path`, `file_size`,
    `mime_type`, `uploaded_by`, `created_by`, `created_at`, `updated_by`, `updated_at`,
    `deleted_by`, `deleted_at`
  - Factory method `create(result_id, file_name, storage_path, file_size, mime_type,
    uploaded_by)` with domain validation:
    - `file_name` must not be empty after trimming; directory components already
      stripped at the boundary (the entity receives the sanitized name)
    - `file_path` must not start with `/` or contain `..`
    - `file_size` must be > 0
    - `mime_type` must not be empty and must contain a `/` character
  - No framework imports; pure language struct + impl
  - Implement `is_soft_deleted()` convenience method

  **Value objects:**
  - `AllowedMimeType`: wraps a MIME type string, validates against the configured
    `ALLOWED_MIME_TYPES` list. Implements `Display` and equality traits.
  - `FileSize`: wraps `u64`. Validates `> 0` and `<= MAX_FILE_SIZE_BYTES`
    (limit is injected or read from config at construction time).
  - `FileName`: wraps a sanitized original file name. Validates not empty after trim,
    strips directory components (keeps only base name via `Path::file_name()` or
    equivalent), max 255 chars.

  **Domain exceptions:**
  - `ResultFileNotFoundError`
  - `ResultFileTooLargeError` (carries the actual size and the limit)
  - `ResultFileTypeNotAllowedError` (carries the rejected MIME type)
  - `ResultFileEmptyNameError` (original file name is empty after sanitization)
  - `ResultFileDiskError` (carries the underlying I/O error; for disk full, permission
    denied, etc.)
  - `ResultFilePermissionDenied` (carries the reason: missing system permission or
    not a project member)

---

## Layer 2 -- Application

- [ ] 2. Define interfaces, ports, and DTOs -- `design.md#Components`, `design.md#API Contract`

  **`TestResultFileRepository` interface (port):**
  - `find_by_id(file_id) -> Option<TestResultFile>` -- returns full row including
    `file_path` (for internal use by the service layer)
  - `find_by_result(result_id) -> Vec<TestResultFileListItem>` -- returns only list
    display fields: `id`, `file_name`, `file_size`, `mime_type`, `uploaded_by`,
    `created_at`
  - `save(file) -> TestResultFile` -- inserts and returns with generated `id` and
    timestamps. Sets `uploaded_by`, `created_by`, `updated_by` all to the same user ID.
  - `soft_delete(file_id, deleted_by)` -- sets `deleted_at` and `deleted_by`;
    returns rows affected (0 if already soft-deleted or not found)
  - All methods accept a transaction context

  **`FileStorage` interface (port):**
  - `store(relative_path: &str, stream: impl Read, max_size: u64) -> Result<u64>` --
    writes file content, returns actual bytes written. Cleans up partial file on error.
  - `exists(relative_path: &str) -> bool`
  - `read(relative_path: &str) -> Result<impl Read>` -- opens file for streaming read

  **DTOs:**
  - `UploadFileCommand` (result_id, file_stream, original_filename, mime_type,
    content_length_hint: Option<u64>)
  - `TestResultFileListItem` (id, file_name, file_size, mime_type, uploaded_by,
    created_at) -- used in list endpoint
  - `TestResultFileDetail` (id, test_case_result_id, file_name, file_size, mime_type,
    uploaded_by, created_by, created_at, updated_by, updated_at) -- used in upload
    response
  - `DownloadFileResult` (file_name, mime_type, file_size, relative_path) -- returned
    by the service to the handler so it can stream the file

- [ ] 3. Implement `TestResultFileService` -- `requirements.md#US-1` through `US-4`,
      `design.md#Sequence`
  - Dependencies: `TestResultFileRepository`, `FileStorage`, `AuthorizationService`,
    `ProjectMemberRepository`, config (`MAX_FILE_SIZE_BYTES`, `ALLOWED_MIME_TYPES`)
  - `list_files(result_id, current_user_id)`: validates the Test Case Result and
    parent Test Execution exist and are not soft-deleted, resolves project scope from
    the execution, checks `test_execution:update` permission, checks project
    membership, delegates to `find_by_result`, returns `Vec<TestResultFileListItem>`
  - `upload_file(cmd: UploadFileCommand, current_user_id)`: validates result/execution
    exist and are not soft-deleted, resolves project scope, checks
    `test_execution:update` permission, checks project membership. Sanitizes original
    file name (strip directory components, validate not empty). Validates MIME type
    against `ALLOWED_MIME_TYPES`. Generates UUID v4 storage filename with original
    extension. Calls `FileStorage::store(storage_path, stream, MAX_FILE_SIZE_BYTES)`.
    On success, constructs entity with actual byte count and calls `repository.save()`.
    On disk error, does not create a DB record. Returns `TestResultFileDetail`
  - `download_file(file_id, current_user_id)`: looks up file. If not found or
    soft-deleted -> error. Resolves parent chain (file -> result -> execution) for
    project scope. Checks `test_execution:update` permission and project membership.
    Verifies file on disk via `FileStorage::exists()`. Returns `DownloadFileResult`
  - `delete_file(file_id, current_user_id)`: looks up file. If not found or
    soft-deleted -> error. Resolves parent chain for project scope. Checks
    `test_execution:update` permission and membership. Calls
    `soft_delete(file_id, current_user_id)`. Does NOT delete the physical file
  - System Admin bypasses all permission and membership checks

- [ ] 4. Write unit tests for `TestResultFileService` -- `requirements.md#US-1` through
      `US-4`
  - Table-driven tests with mock `TestResultFileRepository`, mock `FileStorage`, mock
    `ProjectMemberRepository`, mock `AuthorizationService`
  - Happy path: list files (returns array), upload file (returns metadata), download
    file (returns path for streaming), delete file (soft-deletes)
  - List on non-existent / soft-deleted result -> error
  - List on non-existent / soft-deleted execution -> error
  - List with empty result (no files) -> returns empty array
  - Upload: MIME type not allowed -> `ResultFileTypeNotAllowedError`
  - Upload: file size exceeds limit (Content-Length known) -> `ResultFileTooLargeError`
  - Upload: file size exceeds limit (detected during disk write) -> error
  - Upload: empty file name after sanitization -> `ResultFileEmptyNameError`
  - Upload: original file name with directory components -> stripped to base name
  - Upload: disk write failure -> `ResultFileDiskError`, no DB record created
  - Download: file record not found / soft-deleted / not on disk -> error
  - Delete: file not found / already soft-deleted -> error
  - Delete: physical file NOT deleted
  - Permission denial for each endpoint (missing `test_execution:update`)
  - Project membership denial (not a project member)
  - System Admin can perform all operations regardless of membership
  - UUID v4 generation for storage path is unique and includes original extension
  - Upload sets `uploaded_by` and `created_by` to the same user

---

## Layer 3 -- Adapters (HTTP)

- [ ] 5. Implement `TestResultFileHandler` -- `design.md#API Contract`, `design.md#Components`
  - Four handler methods: `list`, `upload`, `download`, `delete`
  - `list`: extract `resultId` from path, call service, serialize as `{"data": [...]}`
  - `upload`: extract `resultId` from path, parse multipart form-data, extract the
    `file` part (reject if missing), extract MIME type from part headers, extract
    original file name from part headers / `Content-Disposition`, pass to service.
    On success, set `Location: /api/v1/files/{id}`, return `201 Created`. Read the
    multipart body with a size limit (`max_file_size + 1 KB` for overhead); if the
    body exceeds this before parsing completes, return `413` immediately
  - `download`: extract `fileId` from path, call service, set `Content-Type`,
    `Content-Disposition: attachment; filename="<name>"` (properly quoted per RFC 6266),
    `Content-Length`, then stream the file content from disk (chunked, not buffered)
  - `delete`: extract `fileId` from path, call service, return `204 No Content`
  - Map domain errors to HTTP status codes:
    - `ResultFilePermissionDenied` -> `403 Forbidden` (generic message)
    - `ResultFileNotFoundError` -> `404 Not Found`
    - `ResultFileTooLargeError` -> `413` (Content-Length known) or `422` (during write)
    - `ResultFileTypeNotAllowedError` -> `422` with `FILE_TYPE_NOT_ALLOWED`
    - `ResultFileEmptyNameError` -> `422` with `VALIDATION_ERROR`
    - `ResultFileDiskError` -> `500 Internal Server Error` (log at ERROR, generic msg)

- [ ] 6. Register routes and write integration tests -- `design.md#Route Registration`,
      `design.md#API Contract`

  **Route registration:**
  - `GET    /api/v1/test-case-results/{resultId}/files`   -> `list`
  - `POST   /api/v1/test-case-results/{resultId}/files`   -> `upload`
  - `GET    /api/v1/files/{fileId}`                       -> `download`
  - `DELETE /api/v1/files/{fileId}`                       -> `delete`
  - All routes require session auth middleware

  **Integration tests** (full request/response with test DB and temp upload dir):
  - Test `200 OK` for list with correct metadata fields
  - Test empty list returns `{"data": []}` and `200 OK`
  - Test `201 Created` for upload with `Location` header
  - Test upload with all 8 allowed file types (PNG, JPEG, PDF, TXT, ZIP, CSV, JSON, XML)
  - Test `422` with `FILE_TYPE_NOT_ALLOWED` for disallowed MIME type
  - Test `413` / `422` for file exceeding size limit
  - Test `422` when no file field provided in multipart form-data
  - Test path traversal sanitization (`../../../etc/passwd` -> base name only)
  - Test Windows path sanitization (backslashes -> base name only)
  - Test download: correct `Content-Type`, `Content-Disposition`, `Content-Length`,
    bit-for-bit identical content
  - Test `404` on download of non-existent / soft-deleted file
  - Test `204` on soft-delete; `404` on repeated delete
  - Test `404` on list/upload for non-existent / soft-deleted result
  - Test `403` on missing permission or non-project-member (each endpoint)
  - Test System Admin bypasses all gates
  - Test file still on disk after soft-delete (physical file preserved)
  - Test a file with a soft-deleted parent result can still be downloaded
  - Test rate limit headers present; `Retry-After` on `429`

---

## Layer 4 -- Infrastructure

- [ ] 7. Create `TEST_RESULT_FILES` database migration -- `design.md#Data Model`
  - Table definition with all columns, PK, FK (`REFERENCES test_case_results(id)
    ON DELETE RESTRICT`), user FKs (`uploaded_by`, `created_by`, `updated_by`,
    `deleted_by` all `REFERENCES users(id) ON DELETE RESTRICT`)
  - `CHECK` constraint: `file_size > 0`
  - `BEFORE UPDATE` trigger `trg_test_result_files_updated_at` setting
    `NEW.updated_at = NOW()`
  - Indexes:
    - `idx_test_result_files_result_id` on `(test_case_result_id) WHERE deleted_at IS NULL`
    - `idx_test_result_files_uploaded_by` on `(uploaded_by)`
    - `idx_test_result_files_created_by` on `(created_by)`
    - `idx_test_result_files_updated_by` on `(updated_by)`
    - `idx_test_result_files_deleted_by` on `(deleted_by)`
  - Rollback: `DROP TRIGGER`, `DROP FUNCTION`, `DROP TABLE`

- [ ] 8. Add file upload configuration -- `design.md#Components`
  - `UPLOAD_DIR` (default `./uploads`, parsed as `PathBuf`)
  - `MAX_FILE_SIZE_BYTES` (default `10485760`, parsed as `u64`)
  - `ALLOWED_MIME_TYPES` (default: PNG, JPEG, PDF, TXT, ZIP, CSV, JSON, XML;
    comma-separated string parsed to `Vec<String>` or `HashSet<String>`)
  - Validate at startup: `UPLOAD_DIR` must exist and be writable (fail fast in
    production, log warning in dev). Must be outside the web root

- [ ] 9. Implement `SqlTestResultFileRepository` -- `design.md#Components`
  - `find_by_id`: `SELECT * FROM test_result_files WHERE id = $1`
    (returns full row incl. `file_path` for internal use; does NOT filter
    `deleted_at` -- the service layer checks for soft-delete)
  - `find_by_result`:
    ```sql
    SELECT id, file_name, file_size, mime_type, uploaded_by, created_at
    FROM test_result_files
    WHERE test_case_result_id = $1 AND deleted_at IS NULL
    ```
  - `save`:
    ```sql
    INSERT INTO test_result_files
      (test_case_result_id, file_name, file_path, file_size, mime_type,
       uploaded_by, created_by, updated_by)
    VALUES ($1, $2, $3, $4, $5, $6, $6, $6)
    RETURNING *
    ```
    Note: `uploaded_by == created_by == updated_by` on creation
  - `soft_delete`:
    ```sql
    UPDATE test_result_files
    SET deleted_at = NOW(), deleted_by = $2, updated_by = $2
    WHERE id = $1 AND deleted_at IS NULL
    ```
    Returns rows affected
  - Use parameterized queries exclusively
  - Write unit tests with a test database (transactional):
    - Insert/retrieve; list by result; soft-delete sets both audit fields; repeated
      soft-delete returns 0; deleted files excluded from list; trigger sets
      `updated_at`; `created_by == uploaded_by == updated_by` on insert

- [ ] 10. Implement `DiskFileStorage` -- `design.md#Components`
  - Implements `FileStorage` using the local filesystem
  - Constructor takes `upload_dir: PathBuf`
  - `store(relative_path, stream, max_size)`:
    - Validate resolved path does not escape `upload_dir` (canonicalize, check prefix)
    - Create parent directories if needed
    - Read stream in chunks (e.g., 8 KB), track cumulative bytes; if exceeds
      `max_size`, delete partial file and return error
    - On I/O error (disk full, permission denied), delete partial file and return error
    - Return total bytes written on success
  - `exists(relative_path)`: safely resolve path, return `metadata().is_ok()`
  - `read(relative_path)`: safely resolve path, open file handle for streaming
  - Write unit tests with `tempfile::TempDir`:
    - Store/read small file with correct content; exceeds max_size -> error + cleanup;
      path traversal attempt -> error; null byte in path -> error; empty file -> error

---

## Cross-Cutting Tasks

- [ ] 11. Wire authorization and dependency injection -- `design.md#Components`,
       `design.md#Authorization`
  - Implement project scope resolution: given a `test_case_result_id`, resolve the
    project by traversing `TEST_CASE_RESULTS -> TEST_EXECUTIONS -> project_id`.
    If the result or execution is not found or soft-deleted, returns appropriate error.
    Write a unit test for correct project ID resolution
  - Register `SqlTestResultFileRepository` as `TestResultFileRepository` implementation
  - Register `DiskFileStorage` as `FileStorage` implementation (with `UPLOAD_DIR`)
  - Register `TestResultFileService` with its dependencies (repository, file storage,
    `AuthorizationService`, `ProjectMemberRepository`, config)
  - Register `TestResultFileHandler` with the service
  - Ensure correct lifetimes/scopes (repository scoped to request, service singleton)

- [ ] 12. Verify rate limiting, write API docs, and manual QA --
       `requirements.md#Security Considerations`, `design.md#API Contract`

  **Rate limiting:**
  - `GET .../results/{resultId}/files` -- 60 req/min group
  - `POST .../results/{resultId}/files` -- 30 req/min group
  - `GET .../files/{fileId}` -- 60 req/min group
  - `DELETE .../files/{fileId}` -- 30 req/min group
  - Verify `X-RateLimit-*` and `Retry-After` headers

  **API documentation:**
  - OpenAPI 3.x spec for all 4 endpoints: paths, methods, parameters, multipart
    form-data schema, all response codes and bodies, authentication, permission
  - Document that `multipart/form-data` must use field name `file`
  - Include curl examples

  **Manual QA checklist:**
  - [ ] List files on a result with no attachments (empty array, 200)
  - [ ] Upload each of 8 allowed types (PNG, JPEG, PDF, TXT, ZIP, CSV, JSON, XML)
  - [ ] Attempt upload of disallowed type (verify 422 `FILE_TYPE_NOT_ALLOWED`)
  - [ ] Attempt upload exceeding max size (verify 413 or 422)
  - [ ] Attempt upload without file field (verify 422)
  - [ ] Upload file with `../` in name (verify stored base name only)
  - [ ] List files after uploading several (verify all appear)
  - [ ] Download file (verify content matches, correct Content-Type and filename)
  - [ ] Download non-existent file ID (verify 404)
  - [ ] Soft-delete file (verify 204)
  - [ ] Download soft-deleted file (verify 404)
  - [ ] Re-delete soft-deleted file (verify 404)
  - [ ] Verify file still on disk after soft-delete
  - [ ] Verify soft-deleted files excluded from list
  - [ ] List/upload on non-existent result ID (verify 404)
  - [ ] Operations without `test_execution:update` permission (verify 403)
  - [ ] Operations without project membership (verify 403)
  - [ ] All operations as System Admin (verify all succeed)
  - [ ] Rate limit headers present on all endpoints
  - [ ] CSRF protection on POST and DELETE
