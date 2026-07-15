# Design: Test Case Result Files

## Architecture

The Test Case Result Files feature follows Clean Architecture layering. File attachments
are sub-resources of Test Case Results, which in turn are children of Test Executions.
All endpoints are authorized under the `test_execution:update` permission, inheriting
access control from the parent result per FR-44.

File content is stored on the filesystem under a configurable upload directory. The
`TEST_RESULT_FILES` table stores only metadata: original file name, storage path, file
size, MIME type, and audit columns.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                        │   │
│  │  - list_files    GET    /api/v1/test-case-results/{rid}/files         │   │
│  │  - upload_file   POST   /api/v1/test-case-results/{rid}/files         │   │
│  │  - download_file GET    /api/v1/files/{fid}                           │   │
│  │  - delete_file   DELETE /api/v1/files/{fid}                           │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  TestResultFileService:                   │  │  - TestResultFile       │  │
│  │  - list_files                             │  │    (entity)             │  │
│  │  - upload_file                            │  │  - AllowedMimeType      │  │
│  │  - download_file                          │  │    (value object)       │  │
│  │  - delete_file                            │  │  - FileSize             │  │
│  │                                           │  │    (value object)       │  │
│  │  Interfaces:                              │  └──────────────────────────┘  │
│  │  - TestResultFileRepository (port)        │                                │
│  │  - FileStorage (port)                     │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlTestResultFileRepository  (implements TestResultFileRepository)  │   │
│  │  - DiskFileStorage              (implements FileStorage)               │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. All file endpoints require an authenticated session (checked by `AuthMiddleware`).
2. All endpoints require the `test_execution:update` system permission, which is the same
   permission used to update the parent Test Case Result. Per FR-44, permission on a Test
   Execution extends to its child results and their file attachments.
3. Project membership is resolved by traversing the full 4-hop parent chain:
   `Test Result File -> Test Case Result -> Test Execution -> Test Run -> Project`
   (through `test_runs.project_id`). The caller must be a project member. System Admin bypasses all checks.
4. File upload validates MIME type and file size against configurable limits before
   writing to disk. Storage file names are UUID-based to prevent collisions and path
   traversal attacks.
5. File download streams content from disk with correct `Content-Type` and
   `Content-Disposition` headers.
6. File delete is a soft-delete on the metadata record. The physical file is preserved
   on disk for audit trail purposes.

---

## API Contract

### Common Error Response Format

All errors follow the standard project format:

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

### GET `/api/v1/test-case-results/{resultId}/files`

List all non-deleted files attached to a Test Case Result.

**Required Permission:** `test_execution:update`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `resultId` | integer | Test Case Result ID |

**Query Parameters:** None.

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "id": 14,
      "file_name": "screenshot-error-500.png",
      "file_size": 245760,
      "mime_type": "image/png",
      "uploaded_by": 15,
      "created_at": "2026-07-15T10:30:00Z"
    },
    {
      "id": 13,
      "file_name": "server-logs.txt",
      "file_size": 8192,
      "mime_type": "text/plain",
      "uploaded_by": 15,
      "created_at": "2026-07-15T10:25:00Z"
    }
  ]
}
```

**Notes:**
- Results are unordered (database insertion order).
- Only non-deleted files are returned (`WHERE deleted_at IS NULL`).
- The response includes `uploaded_by` (numeric user ID) and `created_at` timestamp.
- `file_path` (internal storage path) is never exposed to the client.
- No pagination -- file collections are expected to be small.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_execution:update` permission or is not a project member |
| `404` | `NOT_FOUND` | Test Case Result does not exist, is soft-deleted, or the parent Test Execution does not exist / is soft-deleted |

---

### POST `/api/v1/test-case-results/{resultId}/files`

Upload a single file and attach it to a Test Case Result.

**Required Permission:** `test_execution:update`

**Content-Type:** `multipart/form-data`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `resultId` | integer | Test Case Result ID |

**Form Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file` | file | Yes | The file to upload. Single file per request. |

**Success Response:** `201 Created`

Headers: `Location: /api/v1/files/14`

```json
{
  "data": {
    "id": 14,
    "test_case_result_id": 128,
    "file_name": "screenshot-error-500.png",
    "file_size": 245760,
    "mime_type": "image/png",
    "uploaded_by": 15,
    "created_by": 15,
    "created_at": "2026-07-15T10:30:00Z",
    "updated_by": 15,
    "updated_at": "2026-07-15T10:30:00Z"
  }
}
```

**Notes:**
- The `file_name` stored is the original file name with directory components stripped
  (only the base name is kept).
- The storage file name is a generated UUID v4 with the original extension appended.
- `file_size` is the actual number of bytes written to disk, not the `Content-Length`
  header value.
- File type and size are validated before writing to disk:
  1. MIME type (from the multipart part header) is checked against `ALLOWED_MIME_TYPES`.
  2. If the `Content-Length` header exceeds `MAX_FILE_SIZE_BYTES`, the request is rejected
     before the body is fully read.
  3. After writing to disk, the actual file size is verified against the limit.
- The database record is inserted only after successful file write. If the disk write
  fails, the transaction is rolled back (no orphaned DB record).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_execution:update` permission or is not a project member |
| `404` | `NOT_FOUND` | Test Case Result does not exist, is soft-deleted, or the parent Test Execution does not exist / is soft-deleted |
| `413` | `FILE_TOO_LARGE` | File size exceeds `MAX_FILE_SIZE_BYTES`. The middleware rejects the request before body parsing (if `Content-Length` is too large). If `Content-Length` is missing or spoofed, the application layer rejects after reading. |
| `422` | `FILE_TOO_LARGE` | Fallback: file exceeded size limit after full read (when `Content-Length` was not available or incorrect) |
| `422` | `FILE_TYPE_NOT_ALLOWED` | File MIME type is not in `ALLOWED_MIME_TYPES` |
| `422` | `VALIDATION_ERROR` | No file provided in the multipart form-data |

`413` response body:

```json
{
  "error": {
    "code": "FILE_TOO_LARGE",
    "message": "File size exceeds the maximum allowed size of 10 MB.",
    "details": [
      { "field": "file", "message": "Maximum file size is 10485760 bytes" }
    ]
  }
}
```

`422` response body for invalid MIME type:

```json
{
  "error": {
    "code": "FILE_TYPE_NOT_ALLOWED",
    "message": "File type 'application/octet-stream' is not allowed.",
    "details": [
      { "field": "file", "message": "Allowed types: image/png, image/jpeg, application/pdf, text/plain, application/zip, text/csv, application/json, application/xml" }
    ]
  }
}
```

---

### GET `/api/v1/files/{fileId}`

Download a file by its ID. The response is the raw file content, not JSON.

**Required Permission:** `test_execution:update`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `fileId` | integer | File ID |

**Success Response:** `200 OK`

Headers:
- `Content-Type: <stored mime_type>` (e.g., `image/png`)
- `Content-Disposition: attachment; filename="screenshot-error-500.png"`
- `Content-Length: <file_size>` (bytes)

Body: Raw file content streamed from disk.

**Notes:**
- Authorization is resolved by looking up the file's parent Test Case Result, then
  traversing the full 4-hop chain: file -> Test Case Result -> Test Execution -> Test Run
  -> Project (through `test_runs.project_id`).
- The `Content-Disposition` header uses `attachment` (not `inline`) to force download
  in all browsers. Inline preview can be added in a future phase.
- The original `file_name` is used in the `Content-Disposition` header, properly quoted
  for HTTP header encoding (per RFC 6266).
- Large files are streamed, not buffered entirely in memory.
- If the file exists in the database but not on disk (manual deletion, storage corruption),
  `404 Not Found` is returned and the event is logged at WARN level.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_execution:update` permission or is not a project member |
| `404` | `NOT_FOUND` | File record does not exist, is soft-deleted, or file is missing from disk |

---

### DELETE `/api/v1/files/{fileId}`

Soft-delete a file attachment.

**Required Permission:** `test_execution:update`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `fileId` | integer | File ID |

**Request Body:** None

**Success Response:** `204 No Content`

**Notes:**
- Sets `deleted_at = NOW()` and `deleted_by = <current_user_id>` on the file record.
- The physical file on disk is NOT deleted; it remains for audit trail purposes.
- Repeated DELETE on an already soft-deleted file returns `404 Not Found`.
- Authorization is resolved through the full 4-hop parent chain (file -> result -> execution -> test_run -> project).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_execution:update` permission or is not a project member |
| `404` | `NOT_FOUND` | File record does not exist or is already soft-deleted |

---

## Data Model

### New Table: TEST_RESULT_FILES

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | `BIGINT` | `PK`, `GENERATED ALWAYS AS IDENTITY` | Surrogate primary key |
| `test_case_result_id` | `BIGINT` | `NOT NULL`, `REFERENCES test_case_results(id) ON DELETE RESTRICT` | Each file belongs to exactly one Test Case Result |
| `file_name` | `VARCHAR(255)` | `NOT NULL` | Original file name, sans directory components |
| `file_path` | `VARCHAR(500)` | `NOT NULL` | Relative storage path (UUID-based filename) |
| `file_size` | `BIGINT` | `NOT NULL`, `CHECK (file_size > 0)` | File size in bytes |
| `mime_type` | `VARCHAR(127)` | `NOT NULL` | MIME type from upload (e.g. `image/png`) |
| `uploaded_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | User who uploaded the file |
| `created_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | User who created the record |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Immutable after insert |
| `updated_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | Last user who modified the record |
| `updated_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Updated by `BEFORE UPDATE` trigger |
| `deleted_by` | `BIGINT` | `REFERENCES users(id) ON DELETE RESTRICT` | Nullable; set on soft-delete |
| `deleted_at` | `TIMESTAMPTZ` | | Nullable; set on soft-delete |

**Constraints:**

```sql
-- File size must be positive
ALTER TABLE test_result_files
  ADD CONSTRAINT chk_test_result_files_size
  CHECK (file_size > 0);

-- Foreign key indexes
CREATE INDEX idx_test_result_files_result_id ON test_result_files (test_case_result_id)
  WHERE deleted_at IS NULL;

CREATE INDEX idx_test_result_files_uploaded_by ON test_result_files (uploaded_by);
CREATE INDEX idx_test_result_files_created_by ON test_result_files (created_by);
CREATE INDEX idx_test_result_files_updated_by ON test_result_files (updated_by);
CREATE INDEX idx_test_result_files_deleted_by ON test_result_files (deleted_by);
```

**Design notes:**

- **Dedicated table** (not polymorphic): Each file is explicitly tied to a
  `test_case_result_id`. This avoids the complexity of polymorphic `owner_type`/`owner_id`
  columns and allows proper FK enforcement. If file attachments are needed for other entity
  types in the future, separate dedicated tables should be used, following this same
  pattern.
- **`test_case_result_id` FK with `RESTRICT`** prevents hard-deleting a Test Case Result
  that has attached files. The Test Case Result soft-delete does not cascade.
- **`uploaded_by` vs `created_by`:** Both are set to the same user on creation, but they
  serve different purposes. `created_by` is the audit column for record creation (never
  modified after insert). `uploaded_by` carries the semantic meaning of "who uploaded this
  file" and is included in list/detail responses for display purposes. On creation they
  are identical; the distinction exists for consistency with the broader audit column
  pattern.
- **`file_path`** stores the path relative to the configured `UPLOAD_DIR`. It must never
  start with `/` or contain `..`. The content is a UUID v4 with file extension
  (e.g., `a3f2b1c4-5d6e-7f80-9a0b-cdef12345678.png`).
- **`file_name`** is the sanitized original file name for display and download purposes.
  Directory components are stripped at upload time.
- **`ON DELETE RESTRICT` on user FKs** prevents deleting users who have uploaded files,
  preserving audit trail integrity.
- **`deleted_at` and `deleted_by`** follow the project-wide soft-delete convention:
  both set together on soft-delete, both `NULL` for active records.

### BEFORE UPDATE Trigger

```sql
CREATE OR REPLACE FUNCTION trg_test_result_files_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_test_result_files_updated_at
  BEFORE UPDATE ON test_result_files
  FOR EACH ROW
  EXECUTE FUNCTION trg_test_result_files_updated_at();
```

The trigger ensures `updated_at` is always set to the current timestamp on every `UPDATE`.

### Relationship to Other Tables

```
TEST_EXECUTIONS ──< TEST_CASE_RESULTS ──< TEST_RESULT_FILES >── USERS (uploaded_by)
```

- `TEST_RESULT_FILES.test_case_result_id` -> `TEST_CASE_RESULTS.id`
- `TEST_CASE_RESULTS.execution_id` -> `TEST_EXECUTIONS.id`
- Project scope is resolved by traversing: file -> result -> execution -> test_run -> project
  (through `test_runs.project_id`)

### Filesystem Storage Layout

```
<UPLOAD_DIR>/
├── a3f2b1c4-5d6e-7f80-9a0b-cdef12345678.png
├── b4c5d6e7-8f90-1a2b-3c4d-ef5678901234.txt
└── ...
```

Files are stored flat (no subdirectories) under the configured `UPLOAD_DIR`. The flat
layout avoids directory explosion for the expected scale (hundreds to low thousands of
files per project). If scale demands it in the future, a sharding scheme (e.g.,
`uploads/a3/f2/a3f2b1c4-5d6e-7f80-9a0b-cdef12345678.png`) can be introduced.

---

## Sequence

### List Files Flow

1. Client sends `GET /api/v1/test-case-results/{resultId}/files` with session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler validates the Test Case Result exists and is not soft-deleted.
4. Handler validates the parent Test Execution exists and is not soft-deleted.
5. Handler calls `TestResultFileService::list_files(result_id, current_user_id)`.
6. `TestResultFileService` resolves the project scope via the full 4-hop chain: result
   -> execution -> test_run -> project (through `test_runs.project_id`).
7. `TestResultFileService` checks `test_execution:update` system permission via
   `AuthorizationService`.
8. `TestResultFileService` checks the user is a project member (any role) or System Admin
   via `ProjectMemberRepository`.
9. `TestResultFileService` calls `TestResultFileRepository::find_by_result(result_id)`.
10. Repository queries `SELECT id, file_name, file_size, mime_type, uploaded_by, created_at
    FROM test_result_files WHERE test_case_result_id = $1 AND deleted_at IS NULL`.
11. Handler returns `200 OK` with the list.

### Upload File Flow

1. Client sends `POST /api/v1/test-case-results/{resultId}/files` with multipart form-data
   (field name `file`, e.g., `screenshot.png`) and session cookie.
2. `AuthMiddleware` validates the session and attaches user context.
3. Handler validates the Test Case Result exists, is not soft-deleted, and its parent
   Test Execution exists and is not soft-deleted.
4. Handler calls `TestResultFileService::upload_file(result_id, file_part, current_user_id)`.
5. `TestResultFileService` resolves the project scope via the full 4-hop chain: result
   -> execution -> test_run -> project (through `test_runs.project_id`).
6. `TestResultFileService` checks `test_execution:update` system permission.
7. `TestResultFileService` checks the user is a project member or System Admin.
8. `TestResultFileService` extracts the MIME type from the multipart part header.
9. `TestResultFileService` validates the MIME type against `ALLOWED_MIME_TYPES`. If not
   allowed -> `422 FILE_TYPE_NOT_ALLOWED`.
10. `TestResultFileService` checks the `Content-Length` header (if present) against
    `MAX_FILE_SIZE_BYTES`. If exceeds -> `413 FILE_TOO_LARGE`.
11. `TestResultFileService` extracts the original file name, strips directory components
    (keeps only the base name).
12. `TestResultFileService` generates a UUID v4 storage filename, preserving the original
    extension: `<uuid>.<ext>`.
13. `TestResultFileService` calls `FileStorage::store(storage_path, file_stream, expected_size_limit)`.
    - `DiskFileStorage` creates the file under `<UPLOAD_DIR>/<uuid>.<ext>`.
    - Writes the stream to disk, tracking bytes written.
    - If bytes written exceed the limit, deletes the partial file and returns an error.
    - Returns the actual file size in bytes.
14. If step 13 fails (disk full, permission denied, size exceeded) -> return `500` or `422`
    as appropriate. No DB record is created.
15. `TestResultFileService` constructs a `TestResultFile` entity and calls
    `TestResultFileRepository::save(file)`.
16. Repository inserts the metadata row.
17. Handler constructs the `Location` header (`/api/v1/files/{new_id}`) and returns
    `201 Created`.

### Download File Flow

1. Client sends `GET /api/v1/files/{fileId}` with session cookie.
2. `AuthMiddleware` validates the session and attaches user context.
3. Handler calls `TestResultFileService::download_file(file_id, current_user_id)`.
4. `TestResultFileService` looks up the file via
   `TestResultFileRepository::find_by_id(file_id)`.
5. If the file record does not exist or is soft-deleted -> `404 Not Found`.
6. `TestResultFileService` resolves the full 4-hop parent chain: file -> Test Case Result
   -> Test Execution -> Test Run -> Project (through `test_runs.project_id`), to determine
   the project scope.
7. `TestResultFileService` checks `test_execution:update` system permission.
8. `TestResultFileService` checks the user is a project member or System Admin.
9. `TestResultFileService` resolves the full filesystem path:
   `<UPLOAD_DIR>/<file.file_path>`.
10. `TestResultFileService` calls `FileStorage::exists(path)` to verify the file is on
    disk. If not -> `404 Not Found`.
11. `TestResultFileService` returns the file metadata and filesystem path to the handler.
12. Handler sets response headers:
    - `Content-Type: <file.mime_type>`
    - `Content-Disposition: attachment; filename="<file.file_name>"`
    - `Content-Length: <file.file_size>`
13. Handler streams the file content from disk to the response body.

### Delete File Flow

1. Client sends `DELETE /api/v1/files/{fileId}` with session cookie.
2. `AuthMiddleware` validates the session and attaches user context.
3. Handler calls `TestResultFileService::delete_file(file_id, current_user_id)`.
4. `TestResultFileService` looks up the file via
   `TestResultFileRepository::find_by_id(file_id)`.
5. If the file record does not exist or is already soft-deleted -> `404 Not Found`.
6. `TestResultFileService` resolves the full 4-hop parent chain (file -> result -> execution
   -> test_run -> project) to determine project scope.
7. `TestResultFileService` checks `test_execution:update` system permission.
8. `TestResultFileService` checks the user is a project member or System Admin.
9. `TestResultFileService` calls
   `TestResultFileRepository::soft_delete(file_id, current_user_id)` which sets
   `deleted_at = NOW()`, `deleted_by = current_user_id`.
10. Physical file on disk is NOT deleted.
11. Handler returns `204 No Content`.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `TestResultFile` | Domain (1) | Entity: `id`, `test_case_result_id`, `file_name`, `file_path`, `file_size`, `mime_type`, `uploaded_by`, audit fields. Factory method `create(result_id, file_name, storage_path, file_size, mime_type, uploaded_by)` performs domain validation (file size > 0, MIME type not empty, file name not empty after sanitization). No ORM or framework imports. |
| `AllowedMimeType` | Domain (1) | Value object: wraps a MIME type string. Validates against the configured allowed types list. |
| `FileSize` | Domain (1) | Value object: wraps `u64`. Validates `> 0` and `<= MAX_FILE_SIZE_BYTES`. |
| `TestResultFileService` | Application (2) | Orchestrates all file operations: `list_files`, `upload_file`, `download_file`, `delete_file`. Each method resolves the project scope by traversing the full 4-hop parent chain (result -> execution -> test_run -> project through `test_runs.project_id`), checks `test_execution:update` permission, checks project membership, then delegates to the repository and file storage. |
| `TestResultFileRepository` | Application (2) | Interface (port): `find_by_id(file_id)`, `find_by_result(result_id)`, `save(file)`, `soft_delete(file_id, deleted_by)`. |
| `FileStorage` | Application (2) | Interface (port): `store(path, stream, size_limit) -> u64` (writes file, returns bytes written), `exists(path) -> bool`, `read(path) -> Stream`. Defined in the application layer because the use case depends on it; implementation is in infrastructure. |
| `TestResultFileHandler` | Adapters (3) | HTTP handler with four methods: `list`, `upload`, `download`, `delete`. Deserializes multipart form-data for upload, streams file content for download, serializes JSON responses for list. |
| `SqlTestResultFileRepository` | Infrastructure (4) | Implements `TestResultFileRepository` using PostgreSQL. All queries include `WHERE deleted_at IS NULL` for active record filtering. Uses parameterized queries exclusively. |
| `DiskFileStorage` | Infrastructure (4) | Implements `FileStorage` using the local filesystem. Reads `UPLOAD_DIR` from configuration. Validates that resolved paths do not escape the upload directory (defence in depth against path traversal). |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| HTTP router registration | Register four new routes: two scoped under test-case-results, two at the top-level files path. All require session auth middleware. |
| Config (`pydantic-settings` or equivalent) | Add `UPLOAD_DIR` (default `./uploads`), `MAX_FILE_SIZE_BYTES` (default 10485760), `ALLOWED_MIME_TYPES` (default: 8 types) settings. |
| `AuthorizationService` (Application) | No new permission codes needed -- file operations reuse `test_execution:update`. Verify that the `test_execution:update` permission code is registered in the permission registry. |

---

## Route Registration

```text
# Scoped under test case results (list, upload)
GET    /api/v1/test-case-results/{resultId}/files    -> list
POST   /api/v1/test-case-results/{resultId}/files    -> upload

# Top-level file operations (download, delete)
GET    /api/v1/files/{fileId}                        -> download
DELETE /api/v1/files/{fileId}                        -> delete
```

All routes require:
- Session-based authentication (`AuthMiddleware`)
- System permission check (`test_execution:update`)
- Project membership check (resolved via the full 4-hop chain: file -> Test Case Result
  -> Test Execution -> Test Run -> Project through `test_runs.project_id`)
- For list and upload: Test Case Result and parent Test Execution existence (both must
  exist and not be soft-deleted; Test Run and Project must also exist and not be
  soft-deleted)

---

## Permission Codes

File operations reuse the existing `test_execution:update` permission. No new permission
codes are introduced. This follows FR-44: permission on a Test Execution extends to its
child Test Case Results and their file attachments.

If `test_execution:update` has not yet been seeded, it must be added:

| code | name |
|------|------|
| `test_execution:update` | Update Test Execution |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks `test_execution:update` permission | `403` | `FORBIDDEN` | INFO | Generic message |
| User is not a project member | `403` | `FORBIDDEN` | INFO | Same generic message; indistinguishable from missing permission |
| Test Case Result not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Checked before list, upload operations |
| Parent Test Execution not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Checked before list, upload operations |
| File record not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Checked before download, delete operations |
| File not found on disk | `404` | `NOT_FOUND` | WARN | Physical file missing; database record exists |
| File size exceeds `MAX_FILE_SIZE_BYTES` (Content-Length known) | `413` | `FILE_TOO_LARGE` | INFO | Rejected before reading body |
| File size exceeds `MAX_FILE_SIZE_BYTES` (after full read) | `422` | `FILE_TOO_LARGE` | INFO | Fallback when Content-Length is absent/spoofed |
| MIME type not in `ALLOWED_MIME_TYPES` | `422` | `FILE_TYPE_NOT_ALLOWED` | INFO | Checked from multipart part header |
| No file provided in form-data | `422` | `VALIDATION_ERROR` | INFO | Multipart body missing file field |
| Disk write failure (disk full, permission denied) | `500` | `INTERNAL_ERROR` | ERROR | No DB record inserted |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | All operations fail |
| Unexpected internal failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |
| Rate limit exceeded | `429` | `RATE_LIMITED` | INFO | Returns `Retry-After` header |

**Anti-patterns explicitly avoided:**

- **Do not expose** the internal `file_path` in any API response.
- **Do not store files** inside the web root or serve them via direct URL mapping.
- **Do not use user-supplied data** for filesystem paths -- storage paths are always
  UUID-based.
- **Do not hard-delete** file records or physical files.
- **Do not cascade-delete** files when a Test Case Result is soft-deleted.
- **Do not buffer** large files entirely in memory during upload or download -- use
  streaming.
- **Do not introduce** new permission codes -- reuse `test_execution:update` per FR-44.
- **Do not allow** path traversal in original file names -- strip directory components
  and use UUID-based storage names.
