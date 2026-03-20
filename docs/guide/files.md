# File Sharing

You can share files in any channel or DM by attaching them to a message.

## Uploading

Two ways to attach files:

- **Button** — Click the paperclip icon in the message composer.
- **Drag and drop** — Drag files directly into the composer area.

You can attach up to **10 files per message** (configurable by the admin). Add optional text content alongside the attachments.

## File Size Limits

The default maximum file size is **20 MB** per file. This is configurable by the server administrator via the `storage.max_file_size` setting.

## Supported Files

Most file types are allowed. The following extensions are **blocked** by default for security:

- `.exe`, `.bat`, `.sh`, `.msi`, `.cmd`, `.ps1`

The blocked list is configurable by the admin.

## Previews

- **Images** (JPEG, PNG, GIF, WebP) — Displayed inline in the message with a preview. Image dimensions are extracted on upload to prevent layout shifts.
- **PDFs** — Displayed inline in the browser when clicked.
- **Other files** — Shown as a download link with the filename and size.

## Downloading

Click an attachment to download it. Images and PDFs open inline; other file types trigger a browser download.

## Storage

Files are stored either on the server's local filesystem or in S3-compatible object storage (via the Barbacane gateway). The storage backend is configured by the admin — it's transparent to users.

## Soft Deletion

When a message is deleted, its attachments remain on disk temporarily. A background cleanup job removes orphaned files after the retention period (default: 30 days).
