# Design: Test Case Files

## Architecture

The Test Case Files feature follows Clean Architecture layering. Files are attachments scoped
to a single test case, with endpoints nested under the test case resource path
(`/api/v1/projects/{projectId}/test-cases/{id}/files`). Authorization mirrors the test case
model: Contributors can upload files to their own test cases and delete only files they
themselves uploaded. Owners and Editors can manage all files.

File storage is abstracted behind a `FileStorage` interface (port) defined in the
Application layer. The initial implementation uses the local filesystem. A future S3 or
object-storage backend can be added by implementing the same interface without changing any
application or domain code.

```
┌──────────────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                                  │
│  ┌───────────────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                                │   │
│  │  - list_files    GET    /api/v1/projects/{pid}/test-cases/{id}/files          │   │
│  │  - upload_file   POST   /api/v1/projects/{pid}/test-cases/{id}/files          │   │
│  │  - download_file GET    /api/v1/projects/{pid}/test-cases/{id}/files/{fid}    │   │
│  │  - delete_file   DELETE /api/v1/projects/{pid}/test-cases/{id}/files/{fid}    │   │
│  └──────────────────┬────────────────────────────────────────────────────────────┘   │
│                     │ calls                                                           │
│                     ▼                                                                 │
│  Application (Layer 2)                         ┌──────────────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)                │  │
│  │  TestCaseFileService:                     │  │  - TestCaseFile (entity)        │  │
│  │  - upload_file                            │  │  - FileStorage (port interface) │  │
│  │  - list_files                             │  └──────────────────────────────────┘  │
│  │  - download_file                          │                                        │
│  │  - delete_file                            │                                        │
│  │                                           │                                        │
│  │  Interfaces:                              │                                        │
│  │  - TestCaseFileRepository (port)          │                                        │
│  │  - FileStorage (port)                     │                                        │
│  └──────────┬───────────────────────────────┘                                        │
│             │ delegates to                                                            │
│             ▼                                                                         │
│  Infrastructure (Layer 4)                                                             │
│  ┌───────────────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlTestCaseFileRepository (implements TestCaseFileRepository)               │   │
│  │  - LocalFileStorage (implements FileStorage)                                   │   │
│  └───────────────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. All file endpoints require an authenticated session (checked by `AuthMiddleware`).
2. `POST` (upload) requires the caller to be a Contributor, Editor, or Owner of the project
   (or System Admin). Contributors can only upload to test cases they created
   (`test_case.created_by == current_user_id`).
3. `DELETE` requires the caller to be at least a Contributor. Contributors can only delete
   files they uploaded (`file.uploaded_by == current_user_id`). Owners and Editors can delete
   any file.
4. `GET` (list and download) requires any project membership (or System Admin).
5. Upload validates file size and MIME type before writing to storage. The MIME type is
   detected from file content (magic bytes), not from the client-supplied Content-Type header.
6. Soft-delete on files sets `deleted_at` and `deleted_by` but does not remove the physical
   file from disk. No cascade from test case soft-delete.
7. The `file_path` column is never returned in any API response.

### Security Requirements

**Path traversal prevention:** The `file_name` from the client is sanitized: directory
separators (`/`, `\`) are stripped, and sequences like `..` are rejected. The actual storage
path is server-generated using a UUID and the sanitized file extension, organized as
`{base_upload_dir}/{project_id}/{test_case_id}/{uuid}.{ext}`. No user-supplied text appears
in the storage path.

**MIME type validation:** The MIME type is detected by inspecting the file's magic bytes,
not trusted from the client's `Content-Type` header. Only types in the configured allowlist
are accepted.

**Download safety:** The `Content-Disposition: attachment` header forces browser download
behavior, preventing inline rendering of potentially malicious HTML or SVG files.

**CSRF protection:** All state-changing endpoints (`POST`, `DELETE`) must be protected
against CSRF. Multipart upload requests require CSRF token validation. Same `SameSite=Lax`
cookie policy applies as described in the project-wide security policy.

**Authorization layering:** Three independent authorization gates apply:

1. System permission check (e.g., `test_case_file:upload`)
2. Project membership and role check (Contributor, Editor, Owner for upload/delete; any role
   for reads; Viewer excluded from all mutations)
3. Contributor ownership checks:
   - Upload: verify `test_case.created_by` matches the authenticated user ID
   - Delete: verify `file.uploaded_by` matches the authenticated user ID

System Admin bypasses all gates. See requirements.md Security Considerations for details.

**Rate limiting:** See requirements.md for the per-endpoint rate limit table.

**Configurable limits:** File size limit and MIME type allowlist are defined in application
configuration (e.g., a YAML config file or environment variables). Changing the config takes
effect on the next request.

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

### GET `/api/v1/projects/{projectId}/test-cases/{id}/files`

List all non-deleted files attached to a test case.

**Required Permission:** `test_case_file:read`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Test Case ID |

**Query Parameters:**

| Param | Type | Default | Max | Description |
|-------|------|---------|-----|-------------|
| `search` | string | -- | 255 | Case-insensitive substring match on `file_name` |
| `sort` | string | `-created_at` | -- | Sort field: `file_name`, `-file_name`, `file_size`, `-file_size`, `created_at`, `-created_at` |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "id": 512,
      "file_name": "login-flow-screenshot.png",
      "file_size": 245760,
      "mime_type": "image/png",
      "uploaded_by": 15,
      "created_at": "2026-07-15T10:00:00Z",
      "updated_at": "2026-07-15T10:00:00Z"
    },
    {
      "id": 511,
      "file_name": "test-data.csv",
      "file_size": 1024,
      "mime_type": "text/csv",
      "uploaded_by": 15,
      "created_at": "2026-07-15T09:30:00Z",
      "updated_at": "2026-07-15T09:30:00Z"
    }
  ]
}
```

**Notes:**
- Results are unpaginated (the expected number of files per test case is small, typically
  under 20).
- Excludes soft-deleted files (`WHERE deleted_at IS NULL`).
- Default sort is `-created_at` (newest first).
- `search` applies `ILIKE` on `file_name` when provided; when absent, no filter is applied.
  The search value is escaped: first `\` is doubled to `\\`, then `%` to `\%`, then `_` to
  `\_`. Leading and trailing whitespace is trimmed; an all-whitespace search is treated as
  "no filter."
- The `sort` parameter accepts: `file_name` (A-Z), `-file_name` (Z-A), `file_size`
  (smallest first), `-file_size` (largest first), `created_at` (oldest first),
  `-created_at` (newest first, default).
- `file_path` is never included in the response.
- `updated_by` and `deleted_at`/`deleted_by` are excluded from the list response.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_case_file:read` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist, is soft-deleted, test case does not exist, is soft-deleted, or belongs to a different project |
| `422` | `VALIDATION_ERROR` | Invalid sort parameter or search exceeds 255 characters |

---

### POST `/api/v1/projects/{projectId}/test-cases/{id}/files`

Upload a file attachment to a test case. Single file per request. The request must use
`multipart/form-data` encoding with a single file part named `file`.

**Required Permission:** `test_case_file:upload` AND project role Contributor (own test case
only), Editor, or Owner

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Test Case ID |

**Request:** `multipart/form-data`

| Part Name | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | binary | Yes | The file to upload |

**Success Response:** `201 Created`

Headers: `Location: /api/v1/projects/42/test-cases/128/files/512`

```json
{
  "data": {
    "id": 512,
    "file_name": "login-flow-screenshot.png",
    "file_size": 245760,
    "mime_type": "image/png",
    "uploaded_by": 15,
    "created_at": "2026-07-15T10:00:00Z",
    "updated_at": "2026-07-15T10:00:00Z"
  }
}
```

**Notes:**
- The filename is taken from the multipart part's filename. If the client sends an empty
  filename, `422` is returned.
- `file_size` is the actual number of bytes received (not the Content-Length header).
- `mime_type` is detected from the file's magic bytes, not from the client's Content-Type.
- `file_path` is the server-generated storage location and is never returned in the response.
- Uploaded files are stored under the configured base upload directory with the path pattern
  `{base_upload_dir}/{project_id}/{test_case_id}/{uuid}.{ext}`. The extension is extracted
  from the original filename and sanitized (lowercased, only alphanumeric characters).
- The storage directory is created if it does not exist (with restricted permissions: `0750`
  for directories, `0640` for files).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_case_file:upload` permission, is a Viewer, or (for Contributors) does not own the test case |
| `404` | `NOT_FOUND` | Project does not exist, is soft-deleted, test case does not exist, is soft-deleted, or belongs to a different project |
| `413` | `FILE_TOO_LARGE` | File exceeds the configured maximum size |
| `422` | `VALIDATION_ERROR` | Request has no file part, file part has no filename, or filename is empty |
| `422` | `FILE_TYPE_NOT_ALLOWED` | File MIME type is not in the configured allowlist |
| `422` | `VALIDATION_ERROR` | Filename exceeds 255 characters or contains path traversal sequences |

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

`422` response body for disallowed file type:

```json
{
  "error": {
    "code": "FILE_TYPE_NOT_ALLOWED",
    "message": "File type 'application/x-msdownload' is not allowed. Allowed types: application/pdf, image/png, image/jpeg, text/plain, ...",
    "details": [
      { "field": "file", "message": "File type is not in the allowed list" }
    ]
  }
}
```

---

### GET `/api/v1/projects/{projectId}/test-cases/{id}/files/{fileId}`

Download a file attachment.

**Required Permission:** `test_case_file:read`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Test Case ID |
| `fileId` | integer | File ID |

**Success Response:** `200 OK`

Response body is the raw file bytes. Headers:

| Header | Value |
|--------|-------|
| `Content-Type` | File's `mime_type` (e.g., `image/png`) |
| `Content-Disposition` | `attachment; filename="login-flow-screenshot.png"` |
| `Content-Length` | File's `file_size` in bytes |

**Notes:**
- The file is streamed from the storage backend to the client. Do not buffer the entire file
  in memory.
- The `Content-Disposition` header uses the original `file_name` from the database. The
  filename value is properly quoted for the header per RFC 6266.
- If the physical file is missing from storage, `404` is returned (not `500`), and an ERROR
  is logged with the file ID (but not the storage path).
- Partial content (range requests / `206 Partial Content`) is out of scope for Phase 1.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_case_file:read` permission or is not a project member |
| `404` | `NOT_FOUND` | Project, test case, or file does not exist, is soft-deleted, or belongs to a different project; or physical file is missing from storage |

---

### DELETE `/api/v1/projects/{projectId}/test-cases/{id}/files/{fileId}`

Soft-delete a file attachment. Contributors can only delete files they uploaded.

**Required Permission:** `test_case_file:delete` AND project role Contributor (own files
only), Editor, or Owner

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Test Case ID |
| `fileId` | integer | File ID |

**Request Body:** None

**Success Response:** `204 No Content`

**Notes:**
- Sets `deleted_at = NOW()` and `deleted_by = <current_user_id>`.
- The physical file on disk is NOT deleted. It is preserved for potential recovery.
- For Contributors: the `uploaded_by` field is checked against the authenticated user's ID.
  If they don't match, `403 Forbidden` is returned with a distinct message.
- No referential integrity check is performed before soft-deleting (files are leaf entities).
- Repeated DELETE on an already soft-deleted file returns `404`.
- Soft-deleting the parent test case does NOT cascade to files. Files remain independently
  soft-deleted or active based on their own `deleted_at` status. A file is accessible only
  when both the file and its parent test case are non-deleted.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_case_file:delete` permission, is a Viewer, or (for Contributors) did not upload the file |
| `404` | `NOT_FOUND` | Project, test case, or file does not exist, is soft-deleted, or belongs to a different project |

---

## Data Model

### New Table: TEST_CASE_FILES

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | `BIGINT` | `PK`, `GENERATED ALWAYS AS IDENTITY` | Surrogate primary key |
| `test_case_id` | `BIGINT` | `NOT NULL`, `REFERENCES test_cases(id) ON DELETE RESTRICT` | Parent test case |
| `file_name` | `VARCHAR(255)` | `NOT NULL` | Original filename from the client (sanitized) |
| `file_path` | `VARCHAR(500)` | `NOT NULL` | Server-generated relative storage path |
| `file_size` | `BIGINT` | `NOT NULL` | File size in bytes |
| `mime_type` | `VARCHAR(127)` | `NOT NULL` | MIME type detected from magic bytes |
| `uploaded_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | User who uploaded the file |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Immutable after insert |
| `updated_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Updated by `BEFORE UPDATE` trigger |
| `deleted_by` | `BIGINT` | `REFERENCES users(id) ON DELETE RESTRICT` | Nullable; set on soft-delete |
| `deleted_at` | `TIMESTAMPTZ` | | Nullable; set on soft-delete |

**Constraints:**

```sql
-- Filename must not be empty
ALTER TABLE test_case_files
  ADD CONSTRAINT chk_test_case_files_file_name
  CHECK (char_length(TRIM(file_name)) > 0);

-- File path must not be empty
ALTER TABLE test_case_files
  ADD CONSTRAINT chk_test_case_files_file_path
  CHECK (char_length(TRIM(file_path)) > 0);

-- File size must be positive
ALTER TABLE test_case_files
  ADD CONSTRAINT chk_test_case_files_file_size
  CHECK (file_size > 0);

-- MIME type must not be empty
ALTER TABLE test_case_files
  ADD CONSTRAINT chk_test_case_files_mime_type
  CHECK (char_length(TRIM(mime_type)) > 0);

-- Foreign key indexes
CREATE INDEX idx_test_case_files_test_case_id ON test_case_files (test_case_id);
CREATE INDEX idx_test_case_files_uploaded_by ON test_case_files (uploaded_by);
CREATE INDEX idx_test_case_files_deleted_by ON test_case_files (deleted_by);

-- Composite index for listing active files for a test case
CREATE INDEX idx_test_case_files_active
  ON test_case_files (test_case_id, created_at DESC)
  WHERE deleted_at IS NULL;

-- Composite index for searching file names within a test case
CREATE INDEX idx_test_case_files_name_search
  ON test_case_files (test_case_id, file_name)
  WHERE deleted_at IS NULL;
```

**Design notes:**

- `ON DELETE RESTRICT` on `test_case_id` prevents hard-deleting a test case that still has
  files. Soft-deleting a test case does not cascade to its files (each has its own
  `deleted_at`). File accessibility is gated on both the file's and the test case's
  soft-delete status in application queries.
- `ON DELETE RESTRICT` on `uploaded_by` and `deleted_by` preserves audit trail integrity.
- `file_name` stores the original client-supplied filename (after sanitization: path traversal
  characters stripped, trimmed, validated for length 1-255). This is what appears in the
  `Content-Disposition` header on download.
- `file_path` stores the server-generated storage location (relative to the base upload
  directory). It is NEVER returned in API responses. The path follows the pattern
  `{project_id}/{test_case_id}/{uuid}.{ext}` where `uuid` is a UUID v4 and `ext` is the
  lowercased, alphanumeric-only extension extracted from the original filename.
- `file_size` is set to the actual number of bytes received and written, not the multipart
  Content-Length header (which may be inaccurate for chunked transfers).
- `mime_type` is detected from the file's magic bytes using a content-sniffing library, not
  from the client's Content-Type header.

### BEFORE UPDATE Trigger

```sql
CREATE OR REPLACE FUNCTION trg_test_case_files_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_test_case_files_updated_at
  BEFORE UPDATE ON test_case_files
  FOR EACH ROW
  EXECUTE FUNCTION trg_test_case_files_updated_at();
```

### Relationship to Other Tables

```
PROJECTS ──< TEST_CASES ──< TEST_CASE_FILES
                                │
                                ├── USERS (uploaded_by)
                                └── USERS (deleted_by)
```

- `TEST_CASE_FILES.test_case_id` -> `TEST_CASES.id` (each file belongs to exactly one test
  case)
- `TEST_CASE_FILES.uploaded_by` -> `USERS.id` (the user who uploaded the file)
- `TEST_CASE_FILES.deleted_by` -> `USERS.id` (the user who soft-deleted the file)

---

## Sequence

### Upload File Flow

1. Client sends `POST /api/v1/projects/{projectId}/test-cases/{id}/files` with
   `multipart/form-data` body containing the file part and session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler validates Content-Type is `multipart/form-data`.
4. Handler parses the multipart request, extracting the `file` part. If no file part or
   empty filename -> `422`.
5. Handler reads the file into a buffer up to the configured max size + 1 byte. If the stream
   exceeds the limit -> `413 File Too Large`. If reading fails mid-stream -> `500`.
6. Handler detects the MIME type from the file buffer's magic bytes. If the type is not in
   the allowlist -> `422 File Type Not Allowed`.
7. Handler sanitizes the original filename: trim whitespace, strip directory separators,
   reject empty or path-traversal sequences. If invalid or > 255 chars -> `422`.
8. Handler validates the project exists and is not soft-deleted.
9. Handler validates the test case exists, belongs to the project, and is not soft-deleted.
10. Handler calls `TestCaseFileService::upload_file(project_id, test_case_id, file_buffer,
    file_name, mime_type, file_size, current_user_id)`.
11. `TestCaseFileService` checks `test_case_file:upload` system permission
    (via `AuthorizationService`).
12. `TestCaseFileService` checks the user is a Contributor, Editor, or Owner of the project
    (via `ProjectMemberRepository`). If Viewer or not a member, and not System Admin ->
    `403`.
13. If the user is a Contributor: `TestCaseFileService` fetches the test case and verifies
    `test_case.created_by == current_user_id`. If not matching -> `403` (ownership
    restriction). Owners and Editors skip this check.
14. `TestCaseFileService` generates a unique storage path:
    `{base_upload_dir}/{project_id}/{test_case_id}/{uuid}.{ext}`.
15. `TestCaseFileService` calls `FileStorage::store(file_path, file_buffer)` to persist the
    file to disk.
16. `TestCaseFileService` constructs a `TestCaseFile` entity and calls
    `TestCaseFileRepository::save(test_case_file)` to insert the row. If the DB insert
    fails (e.g., constraint violation, connection error), call
    `FileStorage::delete(storage_path)` to clean up the orphaned file from disk before
    propagating the error.
17. Handler constructs the `Location` header and returns `201 Created`.

### List Files Flow

1. Client sends `GET /api/v1/projects/{projectId}/test-cases/{id}/files?search=screenshot&sort=-created_at`
   with session cookie.
2. `AuthMiddleware` validates the session.
3. Handler validates the project exists and is not soft-deleted.
4. Handler validates the test case exists, belongs to the project, and is not soft-deleted.
5. Handler calls `TestCaseFileService::list_files(project_id, test_case_id, search, sort,
   current_user_id)`.
6. `TestCaseFileService` checks `test_case_file:read` system permission.
7. `TestCaseFileService` checks the user is a member of the project (any role) or System
   Admin.
8. `TestCaseFileService` calls
   `TestCaseFileRepository::find_by_test_case(test_case_id, search, sort)`.
9. Repository executes a query with `WHERE test_case_id = $1 AND deleted_at IS NULL`, optional
   `ILIKE` on `file_name` if `search` is provided, and `ORDER BY` based on `sort`.
10. Handler returns `200 OK` with the file metadata list.

### Download File Flow

1. Client sends `GET /api/v1/projects/{projectId}/test-cases/{id}/files/{fileId}` with
   session cookie.
2. `AuthMiddleware` validates the session.
3. Handler validates the project exists and is not soft-deleted.
4. Handler validates the test case exists, belongs to the project, and is not soft-deleted.
5. Handler calls `TestCaseFileService::download_file(project_id, test_case_id, file_id,
   current_user_id)`.
6. `TestCaseFileService` checks `test_case_file:read` system permission.
7. `TestCaseFileService` checks the user is a project member (any role) or System Admin.
8. `TestCaseFileService` calls `TestCaseFileRepository::find_by_id(file_id)` to get the
   file metadata. If not found or soft-deleted -> error (`404`).
9. `TestCaseFileService` verifies the file's `test_case_id` matches the path parameter. If
   it does not match -> `404` (defence-in-depth; the file belongs to a different test case
   than what the URL suggests).
10. `TestCaseFileService` calls `FileStorage::exists(file_path)` to verify the physical file
    exists. If not -> log ERROR, return `404`.
11. Handler sets `Content-Type`, `Content-Disposition`, `Content-Length` headers and streams
    the file from `FileStorage::open_read_stream(file_path)` to the response body.

### Soft-Delete File Flow

1. Client sends `DELETE /api/v1/projects/{projectId}/test-cases/{id}/files/{fileId}` with
   session cookie.
2. `AuthMiddleware` validates the session.
3. Handler validates the project exists and is not soft-deleted.
4. Handler validates the test case exists, belongs to the project, and is not soft-deleted.
5. Handler calls `TestCaseFileService::delete_file(project_id, test_case_id, file_id,
   current_user_id)`.
6. `TestCaseFileService` checks `test_case_file:delete` system permission.
7. `TestCaseFileService` checks the user is at least a Contributor of the project. If Viewer
   or not a member, and not System Admin -> `403`.
8. `TestCaseFileService` fetches the file by ID. If not found, soft-deleted, or
   `test_case_id` mismatch -> `404`.
9. If the user is a Contributor: `TestCaseFileService` verifies
   `file.uploaded_by == current_user_id`. If not matching -> `403` (ownership restriction).
   Owners and Editors skip this check.
10. `TestCaseFileService` calls
    `TestCaseFileRepository::soft_delete(file_id, current_user_id)` which sets
    `deleted_at = NOW()`, `deleted_by = current_user_id`.
11. Handler returns `204 No Content`.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `TestCaseFile` | Domain (1) | Entity: `id`, `test_case_id`, `file_name`, `file_path`, `file_size`, `mime_type`, `uploaded_by`, `created_at`, `updated_at`, `deleted_by`, `deleted_at`. Factory method `create(test_case_id, file_name, file_path, file_size, mime_type, uploaded_by)` performs domain validation (file_name length 1-255, file_size > 0, mimetype non-empty, file_path non-empty, filename sanitization against path traversal). No ORM or framework imports. |
| `TestCaseFileService` | Application (2) | Orchestrates all file use cases: `upload_file`, `list_files`, `download_file`, `delete_file`. Each method checks the required system permission, project membership, and (for upload/delete by Contributors) ownership. Delegates to repository and file storage. |
| `TestCaseFileRepository` | Application (2) | Interface (port): `find_by_id(file_id)`, `find_by_test_case(test_case_id, search, sort)`, `save(test_case_file)`, `soft_delete(file_id, deleted_by)`. All queries filter `WHERE deleted_at IS NULL` for active records. |
| `FileStorage` | Application (2) | Interface (port): `store(file_path, data)`, `exists(file_path)`, `open_read_stream(file_path)`. Abstracts the underlying storage backend (local FS, S3, etc.). The `file_path` is relative to the storage root. |
| `TestCaseFileHandler` | Adapters (3) | HTTP handler with four methods (`list`, `upload`, `download`, `delete`). Handles multipart parsing for upload, header configuration for download (Content-Disposition, Content-Type), and error mapping. |
| `SqlTestCaseFileRepository` | Infrastructure (4) | Implements `TestCaseFileRepository` using PostgreSQL. All queries use parameterized queries. The `find_by_test_case` query includes `WHERE test_case_id = $1 AND deleted_at IS NULL` and applies search/sort dynamically. |
| `LocalFileStorage` | Infrastructure (4) | Implements `FileStorage` using the local filesystem. Creates directories with `0750` permissions and files with `0640` permissions. `store()` writes atomically (write to temp file, then rename). `open_read_stream()` returns a `Read` handle for streaming. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `AuthorizationService` (Application) | Add permission codes `test_case_file:upload`, `test_case_file:read`, `test_case_file:delete` to the permission registry. Add role-based checks: Contributor/Editor/Owner can upload/delete; all members can read. For upload: Contributor must own the test case. For delete: Contributor must own the file (uploaded_by). |
| Seed migration (Infrastructure, from `iam-permissions`) | Add 3 new permission rows for `test_case_file:*` codes. |
| HTTP router registration | Register four new routes under the test case path. All routes require session auth. |
| Application configuration | Add `max_file_size` (default 10 MB) and `allowed_mime_types` (default list) to the config schema. |

---

## Route Registration

```text
GET    /api/v1/projects/{projectId}/test-cases/{id}/files          -> list
POST   /api/v1/projects/{projectId}/test-cases/{id}/files          -> upload
GET    /api/v1/projects/{projectId}/test-cases/{id}/files/{fileId} -> download
DELETE /api/v1/projects/{projectId}/test-cases/{id}/files/{fileId} -> delete
```

All routes require:
- Session-based authentication (`AuthMiddleware`)
- Project existence validation (project not soft-deleted)
- Test case existence validation (test case exists, belongs to project, not soft-deleted)
- System permission check (respective `test_case_file:*` code)
- Project membership check:
  - `POST`: Contributor (own test case only), Editor, or Owner
  - `DELETE`: Contributor (own files only), Editor, or Owner
  - `GET` (both read endpoints): any project role
- Contributor ownership checks:
  - `POST`: verify `test_case.created_by == current_user_id`
  - `DELETE`: verify `file.uploaded_by == current_user_id`

---

## New Permission Codes

Add to the `PERMISSIONS` table seed data using `INSERT ... ON CONFLICT (code) DO NOTHING`:

| code | name |
|------|------|
| `test_case_file:upload` | Upload Test Case File |
| `test_case_file:read` | Read Test Case File |
| `test_case_file:delete` | Delete Test Case File |

---

## Configuration

The following configuration values must be added to the application config:

```yaml
upload:
  max_file_size: 10485760       # 10 MB in bytes
  base_dir: "/var/data/uploads" # Base directory for file storage
  allowed_mime_types:
    - application/pdf
    - image/png
    - image/jpeg
    - image/gif
    - image/webp
    - text/plain
    - text/csv
    - application/json
    - application/zip
    - application/x-tar
    - application/gzip
    - application/vnd.openxmlformats-officedocument.spreadsheetml.sheet
    - application/vnd.openxmlformats-officedocument.wordprocessingml.document
    - application/vnd.ms-excel
    - application/vnd.ms-powerpoint
```

The `max_file_size` value is used for both application-layer validation and the HTTP
middleware body size limit. The middleware limit should be set slightly higher (e.g.,
max_file_size + 1 KB) to account for multipart overhead, so that the application can return
a clean `413` error rather than the middleware truncating the request.

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks required system permission | `403` | `FORBIDDEN` | INFO | Generic message (same for missing permission and wrong project role) |
| User lacks required project role | `403` | `FORBIDDEN` | INFO | Same generic message |
| Contributor does not own the test case (upload) | `403` | `FORBIDDEN` | INFO | Distinct message: "Contributors can only upload files to their own test cases" |
| Contributor does not own the file (delete) | `403` | `FORBIDDEN` | INFO | Distinct message: "Contributors can only delete files they uploaded" |
| Project not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Checked before any file operation |
| Test case not found, soft-deleted, or wrong project | `404` | `NOT_FOUND` | INFO | Same message regardless of cause |
| File not found, soft-deleted, or wrong test case | `404` | `NOT_FOUND` | INFO | Same message regardless of cause |
| Physical file missing from storage | `404` | `NOT_FOUND` | ERROR | Log the file ID but NOT the file_path |
| File exceeds size limit | `413` | `FILE_TOO_LARGE` | INFO | Checked before writing to disk |
| Empty or missing file in request | `422` | `VALIDATION_ERROR` | INFO | No file part or filename |
| File type not allowed | `422` | `FILE_TYPE_NOT_ALLOWED` | INFO | Detected by magic bytes |
| Filename too long (> 255 chars) | `422` | `VALIDATION_ERROR` | INFO | |
| Filename contains path traversal | `422` | `VALIDATION_ERROR` | INFO | Rejects `../`, `..\\`, etc. |
| Invalid sort value | `422` | `VALIDATION_ERROR` | INFO | |
| Search exceeds 255 characters | `422` | `VALIDATION_ERROR` | INFO | |
| Rate limit exceeded | `429` | `RATE_LIMITED` | INFO | Per-endpoint limits |
| Storage backend write failure | `500` | `INTERNAL_ERROR` | ERROR | Disk full, permission denied, etc. |
| Storage backend read failure | `500` | `INTERNAL_ERROR` | ERROR | File corrupted or inaccessible |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | All operations fail |
| Unexpected internal failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |

**Anti-patterns explicitly avoided:**

- **Do not return `file_path`** in any API response.
- **Do not trust** the client-supplied Content-Type for MIME validation -- use magic bytes.
- **Do not store** files in a web-accessible directory.
- **Do not cascade** test case soft-delete to files.
- **Do not physically delete** files on soft-delete (preserve for recovery).
- **Do not buffer** entire files in memory during download -- stream from storage.
- **Do not expose** internal storage paths in error messages.
- **Do not hard-delete** any record.
- **Do not use** user-supplied filenames in the storage path.
