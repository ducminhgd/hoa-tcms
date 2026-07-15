# Tasks: Test Case Files

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Implement `TestCaseFile` entity -- `requirements.md#US-1`, `design.md#Components`
  - Fields: `id`, `test_case_id`, `file_name`, `file_path`, `file_size`, `mime_type`,
    `uploaded_by`, `created_at`, `updated_at`, `deleted_by`, `deleted_at`
  - Factory method `TestCaseFile::create(test_case_id, file_name, file_path, file_size,
    mime_type, uploaded_by)` with domain validation:
    - `file_name` trimmed, must be 1-255 characters, must not be empty after trim
    - `file_name` must not contain path traversal sequences (`../`, `..\\`) or directory
      separators (`/`, `\`)
    - `file_size` must be > 0
    - `mime_type` must be non-empty after trim
    - `file_path` must be non-empty after trim
  - `is_soft_deleted()` convenience method
  - No framework imports; pure language struct + impl

- [ ] 2. Define domain exceptions for test case files -- `design.md#Error Handling`
  - `TestCaseFileNotFoundError`
  - `FileTooLargeError` (carries the actual size and the max allowed size)
  - `FileTypeNotAllowedError` (carries the detected MIME type and the allowed list)
  - `FileStorageError` (carries the underlying I/O error; wraps storage backend failures)
  - `InvalidFileNameError` (carries the offending filename and the reason: empty, too long,
    path traversal)
  - `TestCaseFilePermissionDenied` (carries the reason: missing system permission, wrong
    project role, contributor test-case ownership restriction, or contributor file ownership
    restriction)
  - `FilePartMissingError` (no file part in multipart request)

---

## Layer 2 -- Application

- [ ] 3. Define `FileStorage` interface (port) -- `design.md#Components`
  - `store(relative_path: &str, data: &[u8]) -> Result<()>` -- writes file to storage
    backend; creates parent directories if needed; writes atomically (temp file + rename)
  - `exists(relative_path: &str) -> Result<bool>` -- checks whether the physical file exists
  - `open_read_stream(relative_path: &str) -> Result<impl Read>` -- opens a streaming read
    handle (must NOT buffer entire file in memory)
  - All methods accept a context for cancellation/timeout
  - `relative_path` is relative to the configured base upload directory

- [ ] 4. Define `TestCaseFileRepository` interface (port) -- `design.md#Components`,
      `design.md#Data Model`
  - Methods:
    - `find_by_id(file_id) -> Option<TestCaseFile>` -- fetches a single file by ID;
      returns `None` if soft-deleted
    - `find_by_test_case(test_case_id, search, sort) -> Vec<TestCaseFileListItem>` --
      returns all non-deleted files for a test case; applies optional `search` ILIKE on
      `file_name` and optional `sort` (whitelist: `file_name`, `-file_name`, `file_size`,
      `-file_size`, `created_at`, `-created_at`)
    - `save(test_case_file) -> TestCaseFileDetail` -- inserts a new row; returns with
      generated `id` and timestamps
    - `soft_delete(file_id, deleted_by)` -- sets `deleted_at = NOW()`,
      `deleted_by = $2` where `id = $1 AND deleted_at IS NULL`; returns number of rows
      affected (0 if already deleted or not found)
  - `TestCaseFileListItem` includes: `id`, `file_name`, `file_size`, `mime_type`,
    `uploaded_by`, `created_at`, `updated_at` (excludes `file_path`, `updated_by`,
    `deleted_by`, `deleted_at`)
  - `TestCaseFileDetail` includes: all fields except `file_path`

- [ ] 5. Define command/query/response DTOs -- `design.md#API Contract`
  - `UploadFileCommand` (test_case_id, file_data, file_name, mime_type, file_size,
    uploaded_by) -- constructed by the handler after parsing the multipart request and
    detecting MIME type
  - `ListFilesQuery` (test_case_id, search?, sort?) -- sort defaults to `-created_at`
  - `TestCaseFileListItem` (id, file_name, file_size, mime_type, uploaded_by, created_at,
    updated_at)
  - `TestCaseFileDetailResponse` (id, file_name, file_size, mime_type, uploaded_by,
    created_at, updated_at, test_case_id)
  - `FileStoragePath` (relative_path: String) -- value object generated from project_id,
    test_case_id, and a UUID v4 with sanitized extension

- [ ] 6. Implement `TestCaseFileService` -- `requirements.md#US-1` through `US-5`,
      `design.md#Sequence`
  - `upload_file(project_id, test_case_id, cmd, current_user_id)`:
    validates `test_case_file:upload` permission via `AuthorizationService`, checks
    Contributor/Editor/Owner project role via `ProjectMemberRepository` (Viewer rejected).
    If Contributor: fetches test case via `TestCaseRepository` (or receives
    `test_case.created_by` from handler) and verifies `created_by == current_user_id`;
    if not owner -> `403`. Generates a unique storage path via
    `FileStoragePath::new(project_id, test_case_id, extension)`. Calls
    `FileStorage::store(path, cmd.file_data)`. Constructs `TestCaseFile` entity, saves
    via `TestCaseFileRepository::save()`, returns `TestCaseFileDetailResponse`.
  - `list_files(project_id, test_case_id, search, sort, current_user_id)`:
    validates `test_case_file:read` permission, checks project membership (any role),
    delegates to `TestCaseFileRepository::find_by_test_case(test_case_id, search, sort)`,
    returns list of `TestCaseFileListItem`.
  - `download_file(project_id, test_case_id, file_id, current_user_id)`:
    validates `test_case_file:read` permission, checks project membership (any role).
    Fetches file by ID. Verifies `file.test_case_id == test_case_id` (defence-in-depth).
    Calls `FileStorage::exists(file.file_path)`. If missing -> log ERROR, return error.
    Returns a tuple of `(TestCaseFile, FileStorageStreamHandle)` for the handler to stream.
  - `delete_file(project_id, test_case_id, file_id, current_user_id)`:
    validates `test_case_file:delete` permission, checks user is at least a Contributor
    (Viewer rejected). Fetches file by ID; verifies `file.test_case_id == test_case_id`.
    If Contributor: verifies `file.uploaded_by == current_user_id`; if not matching ->
    `403` (ownership restriction). Calls `TestCaseFileRepository::soft_delete(file_id,
    current_user_id)`. Does NOT delete the physical file.
  - All methods check system permission via `AuthorizationService`
  - All methods check project membership/role via `ProjectMemberRepository`
  - System Admin bypasses all permission, membership, and ownership checks

- [ ] 7. Write unit tests for `TestCaseFileService` -- `requirements.md#US-1` through `US-5`
  - Table-driven tests with mock `TestCaseFileRepository`, mock `FileStorage`, mock
    `ProjectMemberRepository`, mock `AuthorizationService`, and mock `TestCaseRepository`
    (for fetching `created_by` on upload)
  - Happy path: upload, list, download, delete
  - File size exceeds limit: returns `FileTooLargeError` (app-layer check before storage
    is called; storage mock should NOT be invoked)
  - MIME type not in allowlist: returns `FileTypeNotAllowedError` (app-layer check before
    storage)
  - Permission denial for each permission code (`upload`, `read`, `delete`)
  - Project role denial: Viewer cannot upload/delete; Viewer can read
  - Contributor upload ownership: can upload to own test case
  - Contributor upload ownership restriction: cannot upload to another user's test case
  - Contributor delete ownership: can delete own uploaded file
  - Contributor delete ownership restriction: cannot delete file uploaded by another user,
    even on own test case
  - Owner/Editor can upload to any test case and delete any file
  - File not found returns error
  - Soft-deleted file returns error on download and repeated delete
  - File exists in DB but missing from physical storage: download returns error
  - Storage backend failure (write error): returns `FileStorageError`
  - Storage backend failure (read error): returns `FileStorageError`
  - Search parameter passed through to repository correctly
  - Sort parameter passed through to repository correctly; default sort is `-created_at`
  - Empty filename validation (empty string, whitespace only)
  - Path traversal in filename (`../../../etc/passwd`, `..\\..\\windows`) rejected
  - Filename at exactly 255 chars (passes), 256 chars (rejects)
  - System Admin bypasses project membership and ownership checks
  - File content is correctly base64-encoded test data in mocks (not actual filesystem I/O)

---

## Layer 3 -- Adapters (HTTP)

- [ ] 8. Implement `TestCaseFileHandler` -- `design.md#API Contract`, `design.md#Components`
  - Four handler methods: `list`, `upload`, `download`, `delete`
  - `list`: parse query params `search` (optional, max 255 chars) and `sort` (optional,
    whitelist validation: `file_name`, `-file_name`, `file_size`, `-file_size`,
    `created_at`, `-created_at`; default `-created_at`). Call service, serialize response
    with `200 OK`.
  - `upload`: validate Content-Type is `multipart/form-data`. Parse multipart body,
    extract the `file` part. If no file part or empty filename -> `422`. Read file content
    into a buffer, stopping at `max_file_size + 1` bytes. If exceeds limit -> `413`.
    Detect MIME type from magic bytes (use a content-sniffing library; do NOT trust the
    client-supplied Content-Type header). Sanitize filename: trim, reject path traversal,
    reject empty or > 255 chars. Validate project and test case exist and are non-deleted.
    Construct `UploadFileCommand`, call service, return `201 Created` with `Location` header.
  - `download`: validate project and test case exist and are non-deleted. Call service to
    get file metadata and a read stream. Set response headers: `Content-Type` (file's
    mime_type), `Content-Disposition` (RFC 6266 `attachment; filename="..."` with proper
    quoting), `Content-Length` (file_size). Stream file bytes to response body. Do NOT
    buffer entire file in memory.
  - `delete`: validate project and test case exist and are non-deleted. Call service,
    return `204 No Content`.
  - Map domain/service errors to HTTP status codes and standard error body format:
    - `TestCaseFilePermissionDenied` -> `403 Forbidden` (generic message for system perm /
      project role failures; distinct message for Contributor ownership failure)
    - `TestCaseFileNotFoundError` -> `404 Not Found`
    - `FileTooLargeError` -> `413 Content Too Large` with `FILE_TOO_LARGE`
    - `FileTypeNotAllowedError` -> `422 Unprocessable Entity` with `FILE_TYPE_NOT_ALLOWED`
    - `InvalidFileNameError` -> `422 Unprocessable Entity` with `VALIDATION_ERROR`
    - `FilePartMissingError` -> `422 Unprocessable Entity` with `VALIDATION_ERROR`
    - `FileStorageError` -> `500 Internal Server Error` with `INTERNAL_ERROR`
  - Extract file extension from filename for storage path generation: lowercased,
    alphanumeric characters only; default to empty string if no valid extension

- [ ] 9. Register test case file routes in HTTP router -- `design.md#Route Registration`
  - `GET    /api/v1/projects/{projectId}/test-cases/{id}/files`          -> `list`
  - `POST   /api/v1/projects/{projectId}/test-cases/{id}/files`          -> `upload`
  - `GET    /api/v1/projects/{projectId}/test-cases/{id}/files/{fileId}` -> `download`
  - `DELETE /api/v1/projects/{projectId}/test-cases/{id}/files/{fileId}` -> `delete`
  - All routes require session auth middleware
  - Multipart body size limit middleware: set to `max_file_size + 2048` bytes (account for
    multipart overhead) so the application can return a clean `413` response instead of the
    middleware truncating the body
  - These routes are registered AFTER the test case CRUD routes (they are a sub-group nested
    under test cases)

- [ ] 10. Write integration tests for test case file HTTP handlers -- `design.md#API Contract`
  - Full request/response cycle with a test database (transactional, rolled back) and a
    temporary upload directory
  - Test `201 Created` with `Location` header on successful upload
  - Test uploaded file content round-trips correctly (upload then download)
  - Test `Content-Type`, `Content-Disposition`, and `Content-Length` headers on download
  - Test `200 OK` on list with all file metadata fields (verify `file_path` is NOT in
    response)
  - Test list with search on file_name (verify only matching files)
  - Test list with sort options (file_name asc/desc, file_size asc/desc, created_at
    asc/desc)
  - Test list returns empty array when no files exist
  - Test `204 No Content` on successful delete
  - Test `404 Not Found` on repeated delete
  - Test `404 Not Found` on deleted file download
  - Test `404 Not Found` on deleted file list (excluded from results)
  - Test `413 Content Too Large` when file exceeds max size
  - Test `422` with `FILE_TYPE_NOT_ALLOWED` when MIME type is not in allowlist
  - Test `422` when no file part is sent in the multipart request
  - Test `422` when file part has an empty filename
  - Test `422` when filename contains path traversal (`../screenshot.png`)
  - Test `422` when filename exceeds 255 characters
  - Test `403 Forbidden` on missing permission (each endpoint)
  - Test `403 Forbidden` on Viewer trying to upload/delete
  - Test `403 Forbidden` on Contributor trying to upload to another user's test case
  - Test `403 Forbidden` on Contributor trying to delete a file uploaded by another user
  - Test Contributor can upload to own test case
  - Test Contributor can delete files they uploaded
  - Test Owner/Editor can upload and delete any file
  - Test `404 Not Found` on non-existent project
  - Test `404 Not Found` on non-existent/soft-deleted test case
  - Test `404 Not Found` on non-existent file
  - Test `404 Not Found` on file belonging to a different test case than the URL
  - Test System Admin can perform all operations regardless of membership or ownership
  - Test MIME type detection from file content, not from Content-Type header (send a PNG
    with `Content-Type: text/html` -- should be detected as `image/png`)
  - Test file extension is extracted correctly from filename and lowercased
  - Test storage path format: `{base_upload_dir}/{project_id}/{test_case_id}/{uuid}.{ext}`
  - Test search escaping: `%`, `_`, `\` in search value treated as literals, not ILIKE
    wildcards
  - Test invalid sort values return `422`
  - Test search exceeding 255 characters returns `422`
  - Test `403` responses for system permission / project role failures use the same generic
    message (do not distinguish between failure modes)
  - Test `403` response for Contributor ownership failures uses distinct messages

---

## Layer 4 -- Infrastructure

- [ ] 11. Create `TEST_CASE_FILES` database migration -- `design.md#Data Model`
  - Table definition with all columns, PK, FKs (test_case_id -> test_cases, uploaded_by ->
    users, deleted_by -> users), constraints
  - `CHECK` constraint on `file_name`:
    `char_length(TRIM(file_name)) > 0`
  - `CHECK` constraint on `file_path`:
    `char_length(TRIM(file_path)) > 0`
  - `CHECK` constraint on `file_size`:
    `file_size > 0`
  - `CHECK` constraint on `mime_type`:
    `char_length(TRIM(mime_type)) > 0`
  - FK indexes on `test_case_id`, `uploaded_by`, `deleted_by`
  - Partial composite index `idx_test_case_files_active` on
    `(test_case_id, created_at DESC) WHERE deleted_at IS NULL`
  - Partial composite index `idx_test_case_files_name_search` on
    `(test_case_id, file_name) WHERE deleted_at IS NULL`
  - `BEFORE UPDATE` trigger `trg_test_case_files_updated_at` that sets
    `NEW.updated_at = NOW()`
  - `ON DELETE RESTRICT` on all three FKs (test_case_id, uploaded_by, deleted_by)
  - Rollback migration: `DROP TRIGGER IF EXISTS`, `DROP FUNCTION IF EXISTS`,
    `DROP TABLE IF EXISTS`
  - Verify migration runs AFTER the `TEST_CASES` migration (FK dependency)

- [ ] 12. Add test case file permission codes to the permissions seed migration --
       `design.md#New Permission Codes`
  - Add 3 new rows to the permissions seed data with
    `INSERT ... ON CONFLICT (code) DO NOTHING`:
    `('test_case_file:upload', 'Upload Test Case File')`,
    `('test_case_file:read', 'Read Test Case File')`,
    `('test_case_file:delete', 'Delete Test Case File')`
  - Let the database auto-assign IDs (do NOT hardcode numeric IDs)
  - If permissions are seeded in a single migration file, append to that file
  - Verify idempotency: `ON CONFLICT (code) DO NOTHING`

- [ ] 13. Implement `SqlTestCaseFileRepository` -- `design.md#Components`
  - All methods from `TestCaseFileRepository` interface
  - Every SELECT includes `WHERE deleted_at IS NULL` for active record filtering
  - `find_by_id`:
    ```sql
    SELECT id, test_case_id, file_name, file_path, file_size, mime_type,
           uploaded_by, created_at, updated_at
    FROM test_case_files
    WHERE id = $1 AND deleted_at IS NULL
    ```
  - `find_by_test_case`:
    ```sql
    SELECT id, file_name, file_size, mime_type, uploaded_by, created_at, updated_at
    FROM test_case_files
    WHERE test_case_id = $1 AND deleted_at IS NULL
    ```
    - `search` filter: if provided, add `AND file_name ILIKE $N` with `%` wildcards
      wrapped around the escaped search term. Escape `\`, `%`, `_` in that order.
    - `sort`: validate against whitelist (`file_name`, `file_size`, `created_at` with
      optional `-` prefix for DESC) before building `ORDER BY`. Default: `ORDER BY
      created_at DESC`.
    - No pagination (unpaginated per test case)
    - Explicitly selects only the columns in `TestCaseFileListItem` -- does NOT select
      `file_path`, `deleted_by`, or `deleted_at`
  - `save`:
    ```sql
    INSERT INTO test_case_files (test_case_id, file_name, file_path, file_size,
      mime_type, uploaded_by)
    VALUES ($1, $2, $3, $4, $5, $6)
    RETURNING id, created_at, updated_at
    ```
  - `soft_delete`:
    ```sql
    UPDATE test_case_files
    SET deleted_at = NOW(), deleted_by = $2
    WHERE id = $1 AND deleted_at IS NULL
    RETURNING id
    ```
    Returns the number of rows affected (0 if already deleted)
  - Use parameterized queries exclusively -- never string interpolation
  - Write unit tests with a test database (one test transaction per case):
    - Insert and retrieve a file (verify all fields except file_path)
    - List files for a test case (empty, single, multiple)
    - List with search on file_name (matches, partial match, no match)
    - List with sort (each valid option, default sort)
    - Soft-deleted file excluded from find_by_id and find_by_test_case
    - Soft-delete sets deleted_at and deleted_by
    - Repeated soft-delete returns 0 rows affected / None
    - Search escaping: `%`, `_`, `\` in file_name treated as literals
    - BEFORE UPDATE trigger sets updated_at correctly on soft-delete (updated_at changes
      because an UPDATE was performed)

- [ ] 14. Implement `LocalFileStorage` -- `design.md#Components`
  - Implements the `FileStorage` interface
  - Constructor takes the base upload directory from configuration
  - `store(path, data)`:
    - Resolves the full path by joining base_dir + relative_path
    - Creates parent directories with `0750` permissions (or platform-appropriate
      equivalent) if they don't exist
    - Writes data to a temporary file in the same directory
    - Atomically renames the temp file to the target path (prevents partial writes)
    - Sets file permissions to `0640` (readable by owner and group, not world-readable)
    - If the target file already exists (unlikely with UUID names), returns an error
  - `exists(path)`: checks `std::fs::metadata(path).is_ok()`
  - `open_read_stream(path)`: opens the file in read-only mode, returns a streaming
    `Read` handle (e.g., `std::fs::File`). Does NOT buffer the file into memory.
  - Write unit tests with a temporary directory:
    - Store and verify file content
    - Store creates parent directories automatically
    - Store fails if target file already exists (UUID collision test)
    - Atomic write: simulate a crash mid-write; verify only the temp file exists, not a
      partial target file
    - Exists returns true for existing file, false for missing file
    - Open read stream succeeds for existing file
    - Open read stream fails for missing file

- [ ] 15. Wire authorization for test case file permissions -- `design.md#Components`
  - Register the 3 new `test_case_file:*` permission codes in the permission registry
  - Extend the project-scope authorization guard to accept required project roles:
    - `test_case_file:upload` -> Contributor, Editor, or Owner (Viewer excluded)
    - `test_case_file:read`   -> any project member (all 4 roles)
    - `test_case_file:delete` -> Contributor, Editor, or Owner (Viewer excluded)
  - Implement the Contributor ownership checks as separate guards:
    - `test_case_file:upload` guard: if user's project role is Contributor, verify
      `test_case.created_by == current_user_id`. Owners and Editors skip this check.
    - `test_case_file:delete` guard: if user's project role is Contributor, verify
      `file.uploaded_by == current_user_id`. Owners and Editors skip this check.
  - System Admin bypasses all role checks and both ownership checks
  - Write unit tests: verify each role is correctly accepted/rejected; verify
    Contributor test-case ownership check for upload; verify Contributor file ownership
    check for delete; verify Owner/Editor bypass both ownership checks

---

## Cross-Cutting Tasks

- [ ] 16. Add file upload configuration to application config -- `design.md#Configuration`
  - Add `upload` section to the application config struct/schema:
    - `max_file_size` (u64, default 10485760)
    - `base_dir` (String, default "/var/data/uploads" or a sensible dev default)
    - `allowed_mime_types` (Vec<String>, default list as specified in design.md)
  - Ensure the config is loaded at startup and accessible to the handler and service layers
  - Add a method on the config to check if a given MIME type is allowed:
    `is_mime_type_allowed(mime_type: &str) -> bool`
  - If using environment variable overrides, support that (e.g., `UPLOAD_MAX_FILE_SIZE`,
    `UPLOAD_BASE_DIR`)
  - Write a unit test verifying the default config values are as specified
  - Write a unit test verifying unknown MIME types are correctly rejected

- [ ] 17. Wire dependency injection for test case file components -- `design.md#Components`
  - Register `SqlTestCaseFileRepository` as the implementation of `TestCaseFileRepository`
  - Register `LocalFileStorage` as the implementation of `FileStorage`
  - Register `TestCaseFileService` with its dependencies (`TestCaseFileRepository`,
    `FileStorage`, `AuthorizationService`, `ProjectMemberRepository`,
    `TestCaseRepository` -- for fetching `created_by` on upload ownership check)
  - Register `TestCaseFileHandler` with `TestCaseFileService` and config
  - If using a DI container, ensure correct lifetimes/scopes (repository, storage, and
    service can be transient or request-scoped; handler is typically singleton)
  - If using manual wiring in `main`, add the wiring code in dependency order

- [ ] 18. Add `X-Request-ID` correlation logging to file endpoints -- `design.md#Route
       Registration`
  - If a global middleware already handles `X-Request-ID` / `X-Correlation-ID`, no
    additional work is needed. If not, add it to the file route group.
  - Accept from the client and echo back; generate a UUID v4 if absent
  - Log the request ID on file operations (upload, download, delete) especially for
    storage backend errors

- [ ] 19. Verify rate limit configuration covers file endpoints --
       `requirements.md#Security Considerations`
  - If rate limiting is configured at the route group level, ensure file routes are in
    the correct groups:
    - `GET .../files` (list) -- 60 req/min group
    - `GET .../files/{fileId}` (download) -- 60 req/min group
    - `POST .../files` (upload) -- 30 req/min group
    - `DELETE .../files/{fileId}` -- 30 req/min group
  - If rate limiting is per-endpoint, add the configuration for each of the 4 endpoints
  - Verify `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset` headers are
    present on responses
  - Verify `Retry-After` header is present on `429` responses

- [ ] 20. Manual QA checklist for test case files -- `requirements.md#US-1` through `US-5`
  - [ ] Upload a small text file to a test case (verify 201 + Location header)
  - [ ] Upload an image file (PNG) (verify MIME type detection is correct)
  - [ ] Verify uploaded file appears in the list endpoint
  - [ ] Download the uploaded file (verify content matches original)
  - [ ] Download with correct Content-Type and Content-Disposition headers
  - [ ] Upload file with large size (verify accepted if under limit)
  - [ ] Upload file exceeding max size (verify 413 FILE_TOO_LARGE)
  - [ ] Upload file with disallowed MIME type (verify 422 FILE_TYPE_NOT_ALLOWED)
  - [ ] Upload file with no file part (verify 422)
  - [ ] Upload file with empty filename (verify 422)
  - [ ] Upload file with path traversal in filename (verify 422)
  - [ ] Upload as Contributor to own test case (verify success)
  - [ ] Upload as Contributor to another user's test case (verify 403)
  - [ ] Upload as Owner to any test case (verify success)
  - [ ] Upload as Viewer (verify 403)
  - [ ] List files with search filter (verify only matching)
  - [ ] List files with sort by file_name, file_size, created_at (asc and desc)
  - [ ] Delete file as Contributor (own upload) (verify 204)
  - [ ] Delete file as Contributor (another user's upload) (verify 403)
  - [ ] Delete file as Owner (any file) (verify 204)
  - [ ] Re-delete soft-deleted file (verify 404)
  - [ ] List files after delete (verify deleted file excluded)
  - [ ] Download after delete (verify 404)
  - [ ] Verify file_path is never returned in any API response
  - [ ] Verify storage path format on disk
  - [ ] Verify System Admin can perform all operations
  - [ ] Verify rate limit headers present on all endpoints
  - [ ] Verify file permissions on disk are `0640`
  - [ ] Verify directory permissions on disk are `0750`
  - [ ] Verify file content integrity (upload known bytes, download, compare checksum)
