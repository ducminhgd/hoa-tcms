# Tasks: UI — File Upload

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit of work.

---

## Types & Validation

- [ ] 1. Define TypeScript types — `design.md#Component API`
  - `FileUploadConfig`, `UploadedFile`, `FileUploadProps`
  - `FileValidationError`, `QueueItem`
  - `UploadStatus` type: `"queued" | "uploading" | "completed" | "failed"`
  - File: `src/ui/components/FileUpload/types.ts`

- [ ] 2. Implement validation logic — `requirements.md#Client-Side Validation Rules`,
      `design.md#Validation Logic`
  - `validateFiles(files, config, currentCount)` pure function
  - Check each file: max size, allowed MIME types, allowed extensions, max file count
  - Return `{ valid: File[], errors: FileValidationError[] }`
  - Each error includes the file name in the message
  - Edge cases: empty file list, zero maxFiles, empty allowed types (allow all),
    duplicate file names (allowed — different files can have the same name)
  - File: `src/ui/components/FileUpload/validateFiles.ts`

- [ ] 3. Implement upload function — `requirements.md#US-3`,
      `design.md#Upload Method: XMLHttpRequest`
  - `uploadFile(file, uploadUrl, csrfToken?, onProgress?, signal?)` async function
  - Use `XMLHttpRequest` with `FormData` for progress tracking
  - `onProgress` callback receives 0–100 integer
  - Support cancellation via `AbortSignal` (abort XHR when signal fires)
  - Parse successful response as JSON (expect `UploadedFile` from server)
  - Reject with descriptive error on: HTTP error status, network error, abort,
    timeout
  - File: `src/ui/components/FileUpload/uploadFile.ts`

---

## Core Components

- [ ] 4. Implement `DropZone` sub-component — `requirements.md#US-1` and `US-2`,
      `design.md#Drag-and-Drop Handling`
  - Renders drop zone area with dashed border, upload icon, instructional text
  - Renders hidden `<input type="file">` for click-to-browse
  - Drag-and-drop event handlers: `dragenter`, `dragover`, `dragleave`, `drop`
  - Visual state: default, drag-over (highlighted border and background)
  - Use `dragCounter` ref to prevent flicker on child element drag events
  - Click handler delegates to hidden file input
  - `onChange` on file input extracts selected files and calls validation callback
  - Hint text shows supported formats from config
  - When `maxFiles` reached and `disabled` prop is true: greyed out, no pointer events,
    text "Maximum {maxFiles} files reached"
  - File: `src/ui/components/FileUpload/DropZone.tsx`

- [ ] 5. Implement `UploadQueueItem` sub-component — `requirements.md#US-3`,
      `design.md#Visual Specification`
  - Renders single file row: file type icon, file name (truncated with ellipsis),
    file size (formatted KB/MB), progress bar, status indicator
  - Progress bar: 4px height, animating width 0–100%, colour based on status
    (blue for uploading, green for completed, red for failed)
  - Queued state: grey progress bar at 0%, file name and size visible
  - Uploading state: blue progress bar, percentage text
  - Completed state: green progress bar, green checkmark icon
  - Failed state: red progress bar, red X icon, error message below, "Retry" button
  - Format file size: `< 1 KB` → "0.8 KB", `< 1 MB` → "512.3 KB", `>= 1 MB` → "2.1 MB"
  - File: `src/ui/components/FileUpload/UploadQueueItem.tsx`

- [ ] 6. Implement `UploadQueue` sub-component — `design.md#Visual Specification`
  - Renders list of `<UploadQueueItem>` components
  - Footer with: "Clear completed" link (removes all completed items from queue),
    "Retry all failed" link (retries all failed uploads)
  - Queue auto-scrolls to show newest items (scroll to bottom on new file added)
  - File: `src/ui/components/FileUpload/UploadQueue.tsx`

- [ ] 7. Assemble `FileUpload` main component — `requirements.md#US-1` through `US-3`,
      `design.md#Architecture`
  - Compose `<DropZone>` and `<UploadQueue>` in a single container
  - Manage upload queue state: array of `QueueItem` objects
  - On files dropped/selected: run validation, show errors (toast or inline),
    add valid files to queue, begin sequential upload
  - Sequential upload loop: for each queued item, set status to "uploading",
    call `uploadFile()`, update progress, on completion set "completed" and
    call `onUploadComplete`, on failure set "failed" and call `onUploadError`,
    then proceed to next item
  - Handle component unmount: abort all in-progress uploads via stored XHR
    references or `AbortController`
  - Disable drop zone when `currentCount >= config.maxFiles`
  - Expose `retryFile(fileId)` for individual retry via UploadQueueItem
  - Expose `clearCompleted()` to remove completed items from queue
  - File: `src/ui/components/FileUpload/FileUpload.tsx`

---

## Integration

- [ ] 8. Integrate with `test-case-files` page — `design.md#Integration Points`
  - In Test Case detail/edit page, include `<FileUpload>` component
  - Configure: max 5 MB, allowed types: PNG, JPEG, PDF, TXT
  - `uploadUrl` = `/api/v1/test-cases/{testCaseId}/files`
  - `existingFiles` = files already attached from server
  - `onUploadComplete` refreshes the file list
  - (Coordination task — may be partially in `test-case-files` spec)

- [ ] 9. Integrate with `test-case-result-files` page — `design.md#Integration Points`
  - In Test Case Result detail page, include `<FileUpload>` component
  - Configure: max 10 MB, allowed types: PNG, JPEG, PDF, TXT, MP4
  - `uploadUrl` = `/api/v1/test-case-results/{resultId}/files`
  - `existingFiles` = files already attached from server
  - `onUploadComplete` refreshes the file list
  - (Coordination task — may be partially in `test-case-result-files` spec)

---

## Tests

- [ ] 10. Write unit tests — `requirements.md#US-1` through `US-3`,
      `design.md#Validation Logic`
  - Test `validateFiles`: valid files pass through
  - Test `validateFiles`: oversized file rejected with correct message
  - Test `validateFiles`: disallowed MIME type rejected
  - Test `validateFiles`: disallowed extension rejected
  - Test `validateFiles`: max file count enforced
  - Test `validateFiles`: mix of valid and invalid files returns both
  - Test `uploadFile`: progress callback fires with 0–100 values
  - Test `uploadFile`: successful upload resolves with parsed JSON
  - Test `uploadFile`: HTTP error rejects with status code
  - Test `uploadFile`: network error rejects
  - Test `uploadFile`: abort via AbortSignal rejects
  - Test DropZone: dragenter/dragover adds active class
  - Test DropZone: dragleave removes active class
  - Test DropZone: drop extracts files from DataTransfer
  - Test DropZone: click opens file input
  - Test UploadQueue: renders file list with correct statuses
  - Test UploadQueue: "Clear completed" removes completed items
  - Test UploadQueue: "Retry all failed" calls retry for each failed item
  - Test FileUpload: sequential upload processes files one at a time
  - Test FileUpload: max files reached disables drop zone
  - Test FileUpload: unmount aborts in-progress uploads
