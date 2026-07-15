# Feature: UI -- File Upload

## Overview

The File Upload feature provides a reusable drag-and-drop file upload component for the HOA TCMS
frontend. It is consumed by `test-case-files` (attachments to Test Cases) and
`test-case-result-files` (attachments to Test Case Results). The component handles file selection
via drag-and-drop or click-to-browse, client-side validation (file size, MIME type, max file
count), server-side validation feedback (duplicate check on size and type at API level), upload
progress indication (progress bar + percentage), and a file list with remove capability.

No new API endpoints are introduced -- the component integrates with the existing
`POST .../files` (upload) and `DELETE .../files/{fileId}` (remove) endpoints defined in
`test-case-files` and `test-case-result-files`.

Configurable limits (max file size, allowed MIME types, max files per request) are fetched from
the server at mount time, either via a `GET /api/v1/config/upload-limits` endpoint or via
response headers on the first API call, so the component enforces the same constraints
client-side that the server enforces server-side.

---

## User Stories

### US-1: Upload Files via Drag-and-Drop or Click-to-Browse

As a user with upload permission on a Test Case or Test Case Result, I want to upload files by
dragging them onto a drop zone or clicking to browse my filesystem, and see real-time upload
progress, so that I can attach screenshots, logs, or documents with minimal effort.

**Acceptance Criteria (EARS)**

- WHEN the component mounts with an active upload context (valid `uploadUrl` prop provided),
  THE SYSTEM SHALL render a drop zone with a dashed border, an upload icon, the text
  "Drag and drop files here or click to browse", and a hidden `<input type="file">`.
- WHEN one or more files are dragged over the drop zone, THE SYSTEM SHALL apply a visual
  highlight to the drop zone (changed border colour from `#94a3b8` to `#3b82f6`, light blue
  background tint `#eff6ff`) and update the cursor to `copy`.
- WHEN dragged files leave the drop zone without being dropped, THE SYSTEM SHALL remove the
  highlight and revert to the default idle state.
- WHEN files are dropped onto the drop zone, THE SYSTEM SHALL capture the `FileList` from
  `e.dataTransfer.files`, remove the highlight, and run client-side validation on every file.
- WHEN the user clicks anywhere on the drop zone, THE SYSTEM SHALL trigger the hidden file
  input's `click()` to open the native file browser dialog. The file input SHALL set the
  `accept` attribute to the allowed MIME types from config.
- WHEN files are selected via the file browser dialog, THE SYSTEM SHALL process them the same
  way as dropped files (validation then sequential upload).
- AFTER client-side validation passes, THE SYSTEM SHALL upload each valid file sequentially
  (one at a time, single `POST` per file) as `multipart/form-data` with the file in a field
  named `file`.
- WHILE a file is uploading, THE SYSTEM SHALL display a progress bar (4px height, blue fill)
  and a percentage label next to the file name, updating based on `XMLHttpRequest.upload`
  progress events.
- WHEN an upload completes successfully (`201 Created`), THE SYSTEM SHALL replace the progress
  bar with a green checkmark icon, transition the progress bar fill to green (`#22c55e`),
  show "100%", and emit an `onUploadComplete` callback with the server-returned file metadata.
- IF the `uploadUrl` prop is not provided or is empty, THE SYSTEM SHALL render the drop zone
  in a disabled state (greyed out, `opacity: 0.5`, `cursor: not-allowed`, no pointer events)
  with the text "Upload is not available in this context."
- IF the request returns `401 Unauthorized` (session expired mid-upload), THE SYSTEM SHALL
  display an error toast and redirect to the login page.
- IF the request returns `413 Content Too Large`, THE SYSTEM SHALL display the server's error
  message indicating the maximum allowed size on the failed file row.

### US-2: Manage Uploaded Files (View, Remove, Track Status)

As a user managing file attachments, I want to see a list of uploaded files with their file
name, size, and upload status, and be able to remove files I have permission to delete, so that
I can manage attachments attached to my test cases and results.

**Acceptance Criteria (EARS)**

- WHEN the component mounts with initial file data provided via the `initialFiles` prop (an
  array of file metadata objects), THE SYSTEM SHALL render each file as a row in a file list
  below the drop zone, sorted by `created_at` ascending (oldest first, matching the server's
  default sort order).
- Each file row SHALL display: file name (truncated with ellipsis beyond 40 characters, with a
  `title` tooltip showing the full name), file size (formatted as bytes, KB, or MB), upload
  date (relative: "2 minutes ago" or "Jan 15, 2026" on hover via tooltip), and a remove button
  (trash icon).
- WHEN the file list is empty and no upload is in progress, THE SYSTEM SHALL display an
  empty-state message below the drop zone: "No files attached yet." with subdued text styling
  (`#94a3b8`, 14px, italic).
- WHEN the user clicks the remove button on a file row, THE SYSTEM SHALL show a confirmation
  dialog: "Remove [file name]? This action cannot be undone." with "Cancel" and "Remove" buttons
  (the Remove button is destructive-styled, typically red).
- WHEN the user confirms removal, THE SYSTEM SHALL send a `DELETE` request to the URL formed as
  `{deleteUrl}/{fileId}` where `deleteUrl` is composed from the prop or derived from the
  upload context.
- IF the delete request succeeds (`204 No Content`), THE SYSTEM SHALL remove the file row from
  the list with a 300ms fade-out animation and emit an `onFileRemove` callback with the removed
  file's ID.
- IF the delete request fails with `403 Forbidden` (e.g., Contributor trying to delete another
  user's file), THE SYSTEM SHALL display an inline error message on that file row: "You do not
  have permission to remove this file." and keep the file in the list.
- IF the delete request fails with `404 Not Found`, THE SYSTEM SHALL remove the file from the
  local list anyway (the server no longer has it) and display a brief info toast: "File was
  already removed."
- WHEN the upload queue has items, THE SYSTEM SHALL display them above the existing-files list
  with a visual separator (divider line) and a "Pending Uploads" heading, so the user can
  distinguish in-progress files from already-saved files.

### US-3: Receive Client-Side and Server-Side Validation Feedback

As a user uploading files, I want immediate, clear feedback when my file does not meet the size,
type, or count requirements before the upload is attempted, and clear server-side error messages
when the server rejects an upload for duplicate or additional validation reasons, so that I can
fix the problem quickly.

**Acceptance Criteria (EARS)**

- WHEN the component mounts, THE SYSTEM SHALL fetch upload configuration limits from the server:
  `GET /api/v1/config/upload-limits`, which returns `{ maxFileSizeBytes, allowedMimeTypes,
  maxFilesPerRequest }`. The component SHALL cache this response for the page lifetime.
- IF the config endpoint is unavailable (network error, 404, 500), THE SYSTEM SHALL fall back
  to hardcoded defaults (`maxFileSizeBytes = 10485760`, `allowedMimeTypes = ["image/png",
  "image/jpeg", "image/gif", "image/webp", "application/pdf", "text/plain", "application/zip",
  "text/csv", "application/json"]`, `maxFilesPerRequest = 10`) and log a warning to the
  browser console.
- WHEN a file is dropped or selected and its size exceeds `maxFileSizeBytes`, THE SYSTEM SHALL
  reject it immediately **before** any upload request is sent, display an inline validation
  error ("[file name] exceeds the maximum file size of [formatted limit]"), and NOT add it to
  the upload queue.
- WHEN a file is dropped or selected and its MIME type (as reported by `File.type`) does not
  match any entry in the `allowedMimeTypes` list, THE SYSTEM SHALL reject it immediately
  **before** any upload request is sent, display an inline validation error ("[file name] has
  an unsupported file type ([detected MIME type]). Allowed: [extensions list]"), and NOT add
  it to the upload queue.
- WHEN the number of files already uploaded plus the number of files being validated exceeds
  `maxFilesPerRequest`, THE SYSTEM SHALL reject the excess files and display a single inline
  message ("You can upload at most [maxFilesPerRequest] files. [n] file(s) were skipped.").
- WHEN the server returns a validation error for an upload (`422 Unprocessable Entity` with
  `FILE_TOO_LARGE`, `FILE_TYPE_NOT_ALLOWED`, or `DUPLICATE_FILE`), THE SYSTEM SHALL display the
  server's error message on the failed file row as a fallback (client-side validation should
  catch most cases, but the server may perform deeper content inspection or duplicate detection
  on the combination of file size and MIME type).
- WHEN the server returns a duplicate-file error (`409 Conflict` or `422` with
  `DUPLICATE_FILE`), meaning a file with the same name, size, and MIME type already exists,
  THE SYSTEM SHALL display "This file appears to be a duplicate of an existing attachment."
  on the failed file row and NOT add it to the completed list.
- WHEN an upload fails due to a network error (timeout, connection refused) or a `5xx` server
  error, THE SYSTEM SHALL display a "Retry" button next to the failed file row with the error
  message: "Failed to upload [file name]. Network error. Please try again."
- WHEN the user clicks the "Retry" button on a failed file row, THE SYSTEM SHALL re-attempt the
  upload for that specific file only, resetting its progress to 0%.
- IF the response contains a `Retry-After` header (rate limit exceeded, `429 Too Many
  Requests`), THE SYSTEM SHALL pause all remaining queued uploads, display a banner above the
  upload queue: "Rate limit reached. Resuming in [n] seconds...", and automatically resume
  sequential uploads after the specified delay.
- Validation error messages SHALL be displayed inline (not in a toast) directly below the
  offending file row or below the drop zone for files rejected before queueing. Error text
  colour is `#ef4444` (red-500), 12px.

---

## States

The component MUST handle all of the following states correctly:

| State | Visual |
|-------|--------|
| **Empty (Idle)** | Drop zone with dashed border `#94a3b8`, upload icon, instructional text "Drag and drop files here or click to browse". Below it: "No files attached yet." if `initialFiles` is empty, or the existing-files list if files are present. |
| **Dragging** | Drop zone highlighted: border `#3b82f6`, background tint `#eff6ff`, cursor `copy`. Larger upload icon with a slight scale animation. |
| **Uploading** | Drop zone remains active (if under `maxFiles`). Upload queue shows current file with a blue progress bar animating width, percentage text, and remaining files labelled "Pending". |
| **Error** | Failed file rows show red progress bar at the percent reached, red X icon, error message below the row, and a "Retry" button. Drop zone remains active. Rejected-at-validation files show error messages below the drop zone. |
| **Complete** | All queued files processed. Successful files show green progress bar and checkmark. Drop zone returns to idle or, if `maxFiles` reached, transitions to a disabled state with text "Maximum [n] files reached." |
| **Disabled** | Drop zone is greyed out (`opacity: 0.5`, `cursor: not-allowed`, no pointer events). Text: "Upload is not available in this context." or "Maximum [n] files reached." |

---

## Out of Scope

- **Multiple file upload in a single HTTP request** -- files are uploaded sequentially, one
  `POST` per file. Batch multipart upload is Phase 2.
- **Upload cancellation mid-transfer** -- once an upload begins, the user cannot cancel that
  specific file. The browser tab can be closed, which aborts all in-flight XHRs on unmount.
- **File reordering** -- the file list is static and sorted by `created_at`. Drag-to-reorder
  in the file list is not supported.
- **Image/file preview thumbnails** -- only file name, size, status icon, and progress are
  displayed. Inline preview/thumbnails is a separate feature.
- **Paste-from-clipboard** -- pasting screenshots or files from the clipboard is not supported
  in Phase 1.
- **Directory/folder upload** -- `webkitGetAsEntry()` for folder drops is not supported.
  Only individual files can be selected.
- **Resumable/chunked uploads** -- interrupted uploads restart from scratch. No chunked upload
  protocol.
- **File download** -- the component does not render download links. Download is handled by the
  parent page.

---

## Dependencies

- **test-case-files** -- The component is embedded in the Test Case detail/edit page. Uses
  `POST /api/v1/projects/{projectId}/test-cases/{id}/files` for upload and
  `DELETE /api/v1/projects/{projectId}/test-cases/{id}/files/{fileId}` for removal.
- **test-case-result-files** -- The component is embedded in the Test Case Result detail page.
  Uses `POST /api/v1/test-case-results/{resultId}/files` for upload and
  `DELETE /api/v1/files/{fileId}` for removal.
- **iam-auth** -- Authentication state. All upload/delete requests carry the session cookie.
  Component disables the drop zone when the user is not authenticated.
- **auth-rbac** -- Permission state determines whether the remove button is shown for each file
  (Contributors can only delete their own files; Owners/Editors can delete any file).
- **Configuration endpoint** (`GET /api/v1/config/upload-limits`) -- Returns
  `{ maxFileSizeBytes, allowedMimeTypes, maxFilesPerRequest }`. If this endpoint does not yet
  exist, the component uses hardcoded defaults and logs a console warning.
