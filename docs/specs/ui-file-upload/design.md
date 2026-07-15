# Design: UI -- File Upload

## Architecture

The File Upload component is a **frontend-only** reusable component composed of two
sub-components (`DropZone` and `FileList`) and backed by a pure validation module, an
XMLHttpRequest-based upload module, and a config-fetching hook. It receives an upload URL
and a delete URL prefix as props; it manages drag-and-drop events, file selection,
client-side validation, sequential upload queue, and progress tracking internally.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  UI Layer                                                                     │
│                                                                               │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  Parent Page (e.g., TestCaseDetailPage)                                │   │
│  │                                                                        │   │
│  │  ┌──────────────────────────────────────────────────────────────────┐  │   │
│  │  │  <FileUpload                                                        │   │
│  │  │    uploadUrl="/api/v1/projects/1/test-cases/42/files"              │  │   │
│  │  │    deleteUrl="/api/v1/projects/1/test-cases/42/files"              │  │   │
│  │  │    initialFiles={[...]}                                             │  │   │
│  │  │    onUploadComplete={fn}                                            │  │   │
│  │  │    onFileRemove={fn}                                                │  │   │
│  │  │  />                                                                 │   │
│  │  │                                                                      │   │
│  │  │  ┌───────────────────────────────────────────────────────────────┐ │  │   │
│  │  │  │  <DropZone>                                                     │ │   │
│  │  │  │    onFilesSelected={fn}   config={...}   disabled={bool}       │ │   │
│  │  │  │                                                                │ │   │
│  │  │  │  ┌──────────────────────────────────────────────────┐         │ │   │
│  │  │  │  │  ☁️  Drag and drop files here, or click to browse │         │ │   │
│  │  │  │  │     Supported: .pdf, .png, .jpg (max 5 MB)       │         │ │   │
│  │  │  │  └──────────────────────────────────────────────────┘         │ │   │
│  │  │  └───────────────────────────────────────────────────────────────┘ │   │
│  │  │                                                                      │   │
│  │  │  ┌───────────────────────────────────────────────────────────────┐ │  │   │
│  │  │  │  <FileList>         (combined upload queue + existing files)    │ │   │
│  │  │  │                                                                │ │   │
│  │  │  │  Upload Queue:                                                  │ │   │
│  │  │  │  ┌──────────────────────────────────────────────────────────┐  │ │   │
│  │  │  │  │  📄 report.pdf      2.1 MB  ████████████░░░░  75%         │  │ │   │
│  │  │  │  │  🖼️ screenshot.png  1.4 MB  ████████████████  100%  ✅    │  │ │   │
│  │  │  │  │  📄 data.csv        3.2 MB  ████████████████  100%  ❌    │  │ │   │
│  │  │  │  │           Upload failed: network error. Retry              │  │ │   │
│  │  │  │  └──────────────────────────────────────────────────────────┘  │ │   │
│  │  │  │                                                                  │ │   │
│  │  │  │  Attached Files:                                                 │ │   │
│  │  │  │  ┌──────────────────────────────────────────────────────────┐  │ │   │
│  │  │  │  │  📄 test-plan.pdf   1.2 MB  2 minutes ago          🗑️     │  │ │   │
│  │  │  │  │  📄 notes.txt       0.3 KB  5 minutes ago          🗑️     │  │ │   │
│  │  │  │  └──────────────────────────────────────────────────────────┘  │ │   │
│  │  │  └───────────────────────────────────────────────────────────────┘ │   │
│  │  └──────────────────────────────────────────────────────────────────┘  │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                                                                               │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  useUploadConfig() — custom hook                                       │   │
│  │  - Fetches GET /api/v1/config/upload-limits on mount                   │   │
│  │  - Falls back to hardcoded defaults if endpoint is unavailable         │   │
│  │  - Returns { maxFileSizeBytes, allowedMimeTypes, maxFilesPerRequest }  │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                                                                               │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  validateFiles() — pure function                                        │   │
│  │  - Checks each file: size, MIME type, max count versus limits          │   │
│  │  - Returns { valid: File[], errors: FileError[] }                      │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                                                                               │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  uploadFile() — async function (XMLHttpRequest-based)                   │   │
│  │  - Creates FormData, sends POST to uploadUrl                           │   │
│  │  - Reports progress via onProgress(percent) callback                   │   │
│  │  - Supports abort via AbortSignal                                      │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. The parent page obtains limits (from server-side config endpoint or server-rendered
   page data) and passes them to `<FileUpload>` via the required `limits` prop.
2. `<DropZone>` listens for drag events and click-to-browse. When files are received,
   it calls the parent's `onFilesSelected` callback with the raw `File[]`.
3. `<FileUpload>` runs `validateFiles(files, config, currentCount)`. Valid files are
   appended to the upload queue; invalid files trigger inline error messages.
4. `<FileUpload>` processes the queue sequentially: for each `QueueItem`, it calls
   `uploadFile(file, uploadUrl, onProgress, signal)`. Progress updates flow to
   `<FileList>` which re-renders the corresponding `<FileRow>`.
5. On completion (`201`), the file metadata is emitted via `onUploadComplete` and the
   queue item transitions to status `"completed"`.
6. On failure, the queue item transitions to status `"failed"` with the error message
   and a retry button.
7. Existing files from `initialFiles` are rendered in the attached-files section.
   Each row has a remove button that calls `DELETE {deleteUrl}/{fileId}`.
8. On unmount, all in-progress XHRs are aborted via their stored `AbortController`
   references.

---

## Component API

### `<FileUpload>` Props

```typescript
interface UploadLimits {
  maxFileSizeBytes: number;
  allowedMimeTypes: string[];
  maxFilesPerRequest: number;
}

interface ExistingFile {
  id: string;
  file_name: string;
  file_size: number;
  mime_type: string;
  uploaded_by: number;
  created_at: string;
}

interface FileUploadProps {
  /** POST endpoint for file upload (multipart/form-data).  */
  uploadUrl: string;

  /** Base URL for DELETE requests. Appended with /{fileId}.  */
  deleteUrl: string;

  /** Already-uploaded files to display in the attached-files list.  */
  initialFiles: ExistingFile[];

  /** Upload limits. REQUIRED. Passed as server-rendered props from the parent page.
   *  The parent page is responsible for obtaining limits (e.g. from server-side
   *  config endpoint or server-rendered page data). No client-side fallback.  */
  limits: UploadLimits;

  /** Called after each successful upload with the server-returned metadata.  */
  onUploadComplete: (file: ExistingFile) => void;

  /** Called when a previously-uploaded file is successfully removed.  */
  onFileRemove: (fileId: string) => void;

  /** Optional CSRF token included as X-CSRF-Token header on POST/DELETE.  */
  csrfToken?: string;

  /** Whether the upload area should be disabled (e.g., user lacks permission).  */
  disabled?: boolean;

  /** Reason to show in tooltip when disabled.  */
  disabledReason?: string;
}
```

### `<DropZone>` Props

```typescript
interface DropZoneProps {
  /** Called with valid File objects selected via drag-drop or browse.  */
  onFilesSelected: (files: File[]) => void;

  /** Upload limits for accept attribute and hint text.  */
  limits: UploadLimits;

  /** Whether the drop zone is interactive.  */
  disabled: boolean;

  /** Tooltip text shown when disabled.  */
  disabledReason?: string;
}
```

### Internal Types

```typescript
type UploadStatus = "queued" | "uploading" | "completed" | "failed";

interface QueueItem {
  /** Client-side unique ID (nanoid or incrementing counter).  */
  id: string;
  /** The browser File object.  */
  file: File;
  /** 0–100.  */
  progress: number;
  status: UploadStatus;
  /** Server-returned metadata on success.  */
  uploadedFile?: ExistingFile;
  /** Error message on failure.  */
  error?: string;
  /** Whether this error is retryable (network/server errors are; 4xx are not).  */
  retryable: boolean;
  /** AbortController for cancelling in-flight upload.  */
  abortController?: AbortController;
}

interface FileValidationError {
  fileName: string;
  message: string;
}
```

### `useUploadConfig()` Hook

```typescript
function useUploadConfig(overrides?: UploadLimits): {
  limits: UploadLimits;
  loading: boolean;
  error: string | null;
};
```

**Behavior:**
- If `overrides` is provided, return it immediately (no fetch).
- Otherwise, `GET /api/v1/config/upload-limits`, parse JSON, return the limits.
- This hook is used **by the parent page**, not by `<FileUpload>` itself. The parent page
  fetches the config and passes it to `<FileUpload>` via the required `limits` prop.
- Cache the result for the component lifetime (no refetch).

---

## States, Events, and Transitions

### Component States

The `FileUpload` component maintains a union of the following top-level states:

```
                    ┌─────────┐
                    │  idle   │
                    └────┬────┘
                         │ files dropped/selected
                         ▼
                    ┌──────────┐
                    │ dragging │  (transient; reverts to idle on dragleave)
                    └──────────┘
                         │ drop event
                         ▼
               ┌──────────────────┐
               │ validating       │  (synchronous; near-instant)
               └───┬──────────┬───┘
                   │ valid     │ invalid
                   ▼           ▼
             ┌──────────┐  ┌─────────┐
             │uploading │  │  error  │  (files rejected before queue)
             └────┬─────┘  └─────────┘
                  │
          ┌───────┴───────┐
          ▼               ▼
    ┌──────────┐    ┌──────────┐
    │ complete │    │  error   │  (some files failed mid-upload)
    └──────────┘    └────┬─────┘
                         │ retry
                         ▼
                    ┌──────────┐
                    │uploading │  (retry loop)
                    └──────────┘
```

### Event Handlers

| Event | Trigger | Handler |
|-------|---------|---------|
| `dragenter` | File dragged over drop zone | `e.preventDefault()`; increment `dragCounter`; add `is-dragging` CSS class. |
| `dragover` | File dragged within drop zone | `e.preventDefault()` (required to enable drop); keep highlighting. |
| `dragleave` | File dragged out of drop zone | Decrement `dragCounter`; if counter hits 0, remove `is-dragging` class. |
| `drop` | File(s) released on drop zone | `e.preventDefault()`; reset `dragCounter` to 0; remove highlighting; extract `e.dataTransfer.files`; call parent `onFilesSelected(files)`. |
| `click` (drop zone) | User clicks drop zone | Trigger `fileInputRef.current.click()`. |
| `change` (file input) | Files chosen from browser dialog | Extract `e.target.files`; call parent `onFilesSelected(files)`; reset input value to allow re-selection of the same file. |
| `onUploadComplete` (callback) | Server returns 201 | Parent page receives file metadata; refreshes file list or appends to state. |
| `onFileRemove` (callback) | Server returns 204 after delete | Parent page receives removed file ID; updates its state. |
| Component `unmount` | Page navigation or conditional render | Abort all `QueueItem.abortController` instances. |

### Upload Queue Algorithms

**Sequential upload loop (pseudocode):**

```
async function processQueue(queue: QueueItem[], uploadUrl: string, csrfToken?: string):
  for each item in queue where status == "queued":
    item.status = "uploading"
    item.abortController = new AbortController()

    try:
      result = await uploadFile(item.file, uploadUrl, {
        csrfToken: csrfToken,
        onProgress: (pct) => { item.progress = pct; re-render(); },
        signal: item.abortController.signal,
      })
      item.status = "completed"
      item.uploadedFile = result
      item.progress = 100
      emit onUploadComplete(result)

    catch error:
      item.status = "failed"
      item.error = error.message
      item.retryable = isRetryable(error)

    // Pause if rate-limited
    if response had Retry-After header:
      wait for Retry-After seconds
      continue
```

**Retry algorithm:**

```
function retryFile(queueItem: QueueItem):
  queueItem.status = "queued"
  queueItem.progress = 0
  queueItem.error = undefined
  trigger processQueue from current position
```

---

## Config-Driven Limits

### Config Fetching Strategy

1. The **parent page** fetches `GET /api/v1/config/upload-limits` (or reads limits from
   server-rendered page data) and passes them to `<FileUpload>` via the required
   `limits` prop.
2. The `<FileUpload>` component uses the `limits` prop directly; it does not fetch
   limits itself.
3. Expected response (`200 OK`):

   ```json
   {
     "data": {
       "maxFileSizeBytes": 10485760,
       "allowedMimeTypes": [
         "image/png",
         "image/jpeg",
         "image/gif",
         "image/webp",
         "application/pdf",
         "text/plain",
         "text/csv",
         "application/json",
         "application/zip"
       ],
       "maxFilesPerRequest": 10
     }
   }
   ```

4. The `<FileUpload>` component receives limits via its required `limits` prop. The
   parent page is responsible for obtaining limits -- typically from the config endpoint
   above or from server-rendered page data (e.g. injected into a `<script>` tag or
   passed as a prop from a server-side template). There is no client-side hardcoded
   fallback in production code paths: if the parent cannot determine limits, it should
   render the upload area in a disabled state with an appropriate message.

5. The limits are cached in the parent page for its lifetime; no refetch on re-render.

### Alternative: Response Headers

If the `GET /api/v1/config/upload-limits` endpoint is not yet implemented, the component
can alternatively read limits from the `POST .../files` response headers on the first
upload attempt:

| Header | Value |
|--------|-------|
| `X-Upload-Max-Size` | Maximum file size in bytes (e.g., `10485760`) |
| `X-Upload-Allowed-Types` | Comma-separated MIME types |
| `X-Upload-Max-Files` | Maximum total files allowed |

The component should check for these headers on the first successful or 422 response
and cache the values. This is a progressive enhancement -- the config endpoint is preferred.

---

## Validation Logic

```typescript
function validateFiles(
  files: File[],
  limits: UploadLimits,
  currentCount: number,
): { valid: File[]; errors: FileValidationError[] } {
  const valid: File[] = [];
  const errors: FileValidationError[] = [];

  for (const file of files) {
    // 1. Max count check
    if (currentCount + valid.length >= limits.maxFilesPerRequest) {
      const skipped = files.length - valid.length - errors.length;
      errors.push({
        fileName: file.name,
        message: `You can upload at most ${limits.maxFilesPerRequest} files. ` +
          `${skipped} file(s) were skipped.`,
      });
      break; // Stop processing further files once limit is hit
    }

    // 2. Size check
    if (file.size > limits.maxFileSizeBytes) {
      const maxMB = (limits.maxFileSizeBytes / (1024 * 1024)).toFixed(1);
      errors.push({
        fileName: file.name,
        message: `${file.name} exceeds the maximum file size of ${maxMB} MB.`,
      });
      continue;
    }

    // 3. MIME type check (based on browser File.type).
    //    If File.type is empty or falsy, skip client-side MIME validation and defer to
    //    server-side magic-byte validation. Only reject on MIME mismatch when File.type
    //    is non-empty. This prevents rejecting files whose MIME type the browser cannot
    //    determine (e.g., uncommon formats or files without standard extensions).
    if (file.type && limits.allowedMimeTypes.length > 0 &&
        !limits.allowedMimeTypes.includes(file.type)) {
      const extensions = limits.allowedMimeTypes
        .map((m) => m.split("/")[1])
        .map((e) => `.${e}`)
        .join(", ");
      errors.push({
        fileName: file.name,
        message: `${file.name} has an unsupported file type ` +
          `(${file.type}). Allowed: ${extensions}.`,
      });
      continue;
    }

    valid.push(file);
  }

  // Deduplicate: if max count was exceeded for remaining files, add one summary error
  const remainingAfterLoop = files.length - valid.length - errors.length;
  if (remainingAfterLoop > 0 && errors.length > 0) {
    // Already handled in the loop via break
  }

  return { valid, errors };
}
```

**Server-side duplicate check** is handled in the upload response handler, not in
`validateFiles`. If the server returns `409 Conflict` or `422` with code `DUPLICATE_FILE`,
the `FileUpload` component marks the queue item as `"failed"` with a non-retryable error
message: "This file appears to be a duplicate of an existing attachment."

---

## Visual Specification

### Drop Zone

```
┌──────────────────────────────────────────────────────────────────┐
│                                                                  │
│                         ☁️  (upload icon, 48px)                   │
│                                                                  │
│             Drag and drop files here, or click to browse          │
│                                                                  │
│               Supported: .pdf, .png, .jpg (max 5 MB)             │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

| Element | Default (Idle) | Dragging |
|---------|---------------|----------|
| Border | 2px dashed `#94a3b8` | 2px dashed `#3b82f6` |
| Border radius | 8px | 8px |
| Background | `transparent` | `#eff6ff` |
| Padding | 40px 32px | 40px 32px |
| Icon colour | `#64748b` | `#3b82f6` |
| Primary text | 14px, `#475569` | 14px, `#1e40af` |
| Hint text | 12px, `#94a3b8` | 12px, `#60a5fa` |
| Cursor | `pointer` | `copy` |
| Transition | -- | 150ms ease-in-out on border and background |
| Disabled | `opacity: 0.5`, `cursor: not-allowed`, `pointer-events: none` | N/A |

### Upload Queue Item (File Row)

```
┌──────────────────────────────────────────────────────────────────┐
│  📄  report.pdf                  2.1 MB          75%             │
│      ████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░               │
│                                                                  │
│  📄  screenshot.png              1.4 MB          100%     ✅      │
│      ████████████████████████████████████████████               │
│                                                                  │
│  📄  data.csv                    3.2 MB          100%     ❌      │
│      ████████████████████████████████████████████               │
│      Upload failed: network error.                    [Retry]    │
└──────────────────────────────────────────────────────────────────┘
```

| Element | Style |
|---------|-------|
| Row padding | 8px 12px |
| Row border-bottom | 1px solid `#f1f5f9` |
| File type icon | 24px, based on MIME type (PDF icon for PDF, image icon for images, generic doc icon for others). Colour: `#64748b`. |
| File name | 14px, `#1e293b`, `max-width: 260px`, `text-overflow: ellipsis`, `overflow: hidden`, `white-space: nowrap`. Full name on `title` tooltip. |
| File size | 12px, `#64748b`, monospace. Formatted: `< 1 KB` as "0.X KB", `< 1 MB` as "XXX.X KB", `>= 1 MB` as "X.X MB". |
| Progress bar track | `width: 100%`, `height: 4px`, `border-radius: 2px`, `background: #e2e8f0`. |
| Progress bar fill (uploading) | `height: 4px`, `border-radius: 2px`, `background: #3b82f6`, `transition: width 200ms linear`. |
| Progress bar fill (completed) | `background: #22c55e`. |
| Progress bar fill (failed) | `background: #ef4444`. |
| Percentage label | 12px, `#64748b`, monospace, right-aligned. |
| Success checkmark | `#22c55e`, 16px filled circle with white check. |
| Error X icon | `#ef4444`, 16px filled circle with white X. |
| Error message | 12px, `#ef4444`, below progress bar, inline. |
| Retry button | 12px, `#3b82f6`, text button (no border), next to error message. |

### Attached Files Section (Existing Files)

```
┌──────────────────────────────────────────────────────────────────┐
│  Attached Files                                                  │
│                                                                  │
│  📄  test-plan.pdf      1.2 MB    2 minutes ago          🗑️      │
│  📄  notes.txt          0.3 KB    5 minutes ago          🗑️      │
└──────────────────────────────────────────────────────────────────┘
```

| Element | Style |
|---------|-------|
| Section heading | "Attached Files", 14px, `#334155`, `font-weight: 600`, margin-top 16px. |
| Row | Same as upload queue row, without progress bar. |
| Upload date | 12px, `#94a3b8`, relative time. Tooltip on hover shows absolute time. |
| Remove button | 20px trash icon, `#94a3b8`, changes to `#ef4444` on hover. `cursor: pointer`. `aria-label="Remove [file name]"`. |
| Empty state | "No files attached yet.", 14px, `#94a3b8`, italic, centred, padding 24px. |

### Confirmation Dialog (Remove File)

```
┌──────────────────────────────────────────────┐
│  Remove file?                                │
│                                              │
│  Remove test-plan.pdf?                       │
│  This action cannot be undone.               │
│                                              │
│              [Cancel]    [Remove]             │
└──────────────────────────────────────────────┘
```

Standard modal/dialog pattern. The Remove button uses a destructive style (red text or
red outline).

---

## Accessibility Requirements

| Requirement | Implementation |
|-------------|---------------|
| Drop zone is keyboard-operable | The hidden `<input type="file">` is focusable via Tab. Pressing Enter/Space when focused opens the file browser. The drop zone `<div>` has `tabIndex={0}` and an `onKeyDown` handler that opens the file input on Enter/Space. |
| Drag-and-drop alternative | Click-to-browse serves as the full alternative for users who cannot use drag-and-drop (keyboard-only, screen reader, motor impairment). |
| File input label | The hidden file input has an `aria-labelledby` pointing to the drop zone's instructional text, ensuring screen readers announce the purpose. |
| Progress announcements | Each progress update uses an `aria-live="polite"` region to announce percentage changes to screen readers. Debounced to max 1 announcement per second to avoid spamming. |
| Status announcements | Upload completion and failure trigger `aria-live="assertive"` announcements: "report.pdf uploaded successfully" or "Upload failed for data.csv: network error". |
| Remove button labels | Each remove button has `aria-label="Remove [file name]"`. |
| Error messages | Error text is associated with the file row via `aria-describedby`. |
| Colour independence | Upload status is indicated by both colour (green/red/blue) and icon (checkmark/X/spinner). Do not rely on colour alone. |
| Focus management | After a file is removed, focus moves to the next file row or to the drop zone if the list is now empty. |
| Disabled state | When disabled, the drop zone has `aria-disabled="true"` and the reason is in an associated tooltip element. |
| Reduce motion | When `prefers-reduced-motion: reduce` is active, disable the progress bar width transition and the fade-out animation on file removal. |

---

## Edge Cases and Error Handling

| Scenario | Behaviour |
|----------|-----------|
| User drops 50 files at once | Validate each; reject all beyond `maxFilesPerRequest` with a summary message. Queue only the valid N files. |
| User drops a 0-byte file | Treat as valid by size check (0 < maxSizeBytes). Server will likely reject; display server error if so. |
| User drops a file with no extension | Check MIME type only (extension check not applicable). If MIME is allowed, accept. |
| User drops a file with a misleading extension (e.g., `.pdf` renamed from `.exe`) | Client-side trusts the browser's MIME type detection. Server-side MIME validation via magic bytes is the final gate. Component displays the server's 422 error message if rejected. |
| Duplicate file name (same name, different content) | Allowed by client-side validation. Server handles duplicate detection by comparing name + size + MIME type. |
| Network disconnects mid-upload | Mark the in-progress file as "failed" with `retryable: true`. Remaining queued files stay queued. |
| Component unmounts during upload | All in-progress XHRs are aborted immediately via `AbortController.abort()`. Queued files are discarded. |
| CSRF token missing (multipart/form-data POST) | Include `X-CSRF-Token` header. If token is not provided as prop, the request is still sent; the server's CSRF middleware handles enforcement. |
| Session cookie expires between uploads | The first request after expiry returns 401. Component displays a toast and redirects to login. Remaining queued files are not uploaded. |
| Config endpoint returns partial data | If any required field is missing or malformed (e.g., `maxFileSizeBytes` is negative), fall back to the corresponding hardcoded default for that field only. |
| File input `accept` attribute and MIME type list mismatch | The `accept` attribute is set to the `allowedMimeTypes` joined by comma. Browsers use this as a hint only; the validation function is the authoritative check. |

---

## Files

| File | Role |
|------|------|
| `src/ui/components/FileUpload/FileUpload.tsx` | Main component: composes DropZone + FileList, manages queue state, runs sequential upload loop, handles lifecycle. |
| `src/ui/components/FileUpload/DropZone.tsx` | Drag-and-drop zone with click-to-browse. Manages drag events, renders hidden file input, renders instructional text and hint. |
| `src/ui/components/FileUpload/FileList.tsx` | Combined list: "Pending Uploads" queue section + "Attached Files" section. Renders list of FileRow components. |
| `src/ui/components/FileUpload/FileRow.tsx` | Single file row: icon, name, size, progress bar (queue items) or upload date (existing files), status icon, remove button, retry button. |
| `src/ui/components/FileUpload/ConfirmRemoveDialog.tsx` | Confirmation modal for file removal. |
| `src/ui/components/FileUpload/validateFiles.ts` | Pure validation function (size, MIME type, max count). |
| `src/ui/components/FileUpload/uploadFile.ts` | XMLHttpRequest-based upload with progress callback and AbortSignal support. |
| `src/ui/components/FileUpload/useUploadConfig.ts` | Hook: fetches limits from config endpoint, falls back to defaults. |
| `src/ui/components/FileUpload/formatFileSize.ts` | Utility: formats bytes into human-readable string ("1.2 MB", "512.3 KB", "0.8 KB"). |
| `src/ui/components/FileUpload/types.ts` | TypeScript interfaces and type aliases. |
