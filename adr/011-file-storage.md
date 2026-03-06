# ADR-011: File Storage

**Status:** Accepted
**Date:** 2026-03-06

## Context

Per [ADR-002](002-core-feature-set.md), Burst supports file sharing in channels and DMs — images, documents, videos, and other attachments. Per [ADR-003](003-technology-stack.md), file storage uses a local filesystem by default with an S3-compatible backend as an option. Per [ADR-007](007-data-model.md), the `attachments` table stores metadata (file name, size, content type, storage key) while the files themselves live outside the database.

Barbacane already ships an S3 dispatcher plugin that handles SigV4 signing and proxies requests to any S3-compatible store (AWS S3, MinIO, Cloudflare R2, DigitalOcean Spaces). Following the same delegation pattern as authentication ([ADR-006](006-authentication-and-authorization.md)), Burst should delegate S3 operations to Barbacane rather than implementing S3 client logic itself.

## Decision

### Two storage modes

#### Local filesystem (default, no Barbacane needed)

For simple deployments — local disk storage, no S3 credentials to manage:

```
Client ──► Barbacane ──► Burst (handles file I/O directly)
                              │
                              └── reads/writes to local filesystem
```

Burst reads and writes files directly to a configured directory. No S3 SDK, no credentials. This is the zero-config path.

#### S3 via Barbacane dispatcher (recommended for production)

For production deployments — S3 storage delegated entirely to Barbacane:

```
Upload:   Client ──► Barbacane ──► Burst (validates, stores metadata)
                                      │
                                      └──► Barbacane S3 route (PUT to bucket)
                                           (SigV4 signing, endpoint routing)

Download: Client ──► Barbacane S3 route (GET from bucket)
                     (SigV4 signing, auth via ACL/JWT)
```

Burst never talks to S3 directly. All S3 operations go through Barbacane's S3 dispatcher plugin, which handles:

- SigV4 request signing (no AWS credentials in Burst).
- Virtual-hosted and path-style URL construction.
- Custom endpoints for S3-compatible services (MinIO, R2).
- Session token support for IAM roles / IRSA.

This mirrors the auth delegation pattern: S3 configuration lives in the Barbacane spec, not in Burst's config. Switching from local storage to S3 is a gateway configuration change — Burst needs only to know it should route file operations through Barbacane instead of the local filesystem.

### Storage trait

File storage is abstracted behind a trait in `burst-server`:

```rust
#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn put(&self, key: &str, data: Bytes, content_type: &str) -> Result<()>;
    async fn get(&self, key: &str) -> Result<(Bytes, String)>;  // (data, content_type)
    async fn delete(&self, key: &str) -> Result<()>;
    async fn exists(&self, key: &str) -> Result<bool>;
}
```

Two implementations:

- **`LocalStorage`** — reads/writes to a configured directory on the local filesystem.
- **`GatewayStorage`** — proxies file operations to Barbacane's S3 dispatcher via internal HTTP calls (same pattern as the WebSocket auth loopback in [ADR-006](006-authentication-and-authorization.md)).

The trait lives in `burst-server::storage` alongside its implementations — per [ADR-008](008-crate-architecture.md).

### Barbacane spec for S3 routes

The Burst OpenAPI spec includes internal routes for file operations, dispatched by Barbacane's S3 plugin:

```yaml
# In the Barbacane spec (specs/burst-api.yaml)
/storage/{key+}:
  put:
    x-barbacane-dispatch:
      type: s3
      config:
        bucket: burst-uploads
        region: us-east-1
        access_key_id: env://AWS_ACCESS_KEY_ID
        secret_access_key: env://AWS_SECRET_ACCESS_KEY
    # Internal route — not exposed to clients
  get:
    x-barbacane-dispatch:
      type: s3
      config:
        bucket: burst-uploads
        region: us-east-1
        access_key_id: env://AWS_ACCESS_KEY_ID
        secret_access_key: env://AWS_SECRET_ACCESS_KEY
  delete:
    x-barbacane-dispatch:
      type: s3
      config:
        bucket: burst-uploads
        region: us-east-1
        access_key_id: env://AWS_ACCESS_KEY_ID
        secret_access_key: env://AWS_SECRET_ACCESS_KEY
```

The `{key+}` wildcard parameter captures multi-segment paths (e.g., `ch_abc/2026/03/att_def/report.pdf`), which Barbacane's S3 plugin handles natively.

### Storage key format

Files are stored with a structured key that prevents collisions and enables predictable paths:

```
<channel_id>/<year>/<month>/<attachment_id>/<original_filename>
```

Example:
```
ch_01912a3b-4c5d-7e8f-9012-abcdef123456/2026/03/att_01912a3b-9f8e-7d6c-5b4a-fedcba654321/report.pdf
```

- `channel_id` provides logical grouping and enables per-channel storage cleanup.
- `year/month` prevents flat directories with millions of files on local storage.
- `attachment_id` ensures uniqueness (UUIDv7, no filename collisions).
- Original filename is preserved for human readability and correct `Content-Disposition` on download.

### Upload flow

```
Client ──► Barbacane ──► POST /channels/{id}/messages (multipart/form-data)
                              │
                              ├── 1. Validate auth (from X-Auth-Consumer)
                              ├── 2. Validate channel membership
                              ├── 3. Validate file constraints (size, type, count)
                              ├── 4. Stream file(s) to storage backend
                              │      ├── Local: write to disk
                              │      └── S3: PUT via Barbacane S3 route
                              ├── 5. Insert message row in DB
                              ├── 6. Insert attachment row(s) in DB
                              ├── 7. Publish message.created event
                              └── 8. Return message with attachment metadata
```

- **Multipart upload, not separate endpoints.** A message with attachments is a single `POST` — the message content and file(s) are submitted together as `multipart/form-data`. No orphaned files from failed message creation.
- **Stream, don't buffer.** Files are streamed directly from the request body to the storage backend. Burst never holds the entire file in memory.
- **Attachments are always tied to a message.** No standalone file upload endpoint. Every attachment belongs to a message.

### Download / file serving

```
Client ──► Barbacane ──► GET /attachments/{id}
                              │
                              ├── 1. Look up attachment metadata in DB
                              ├── 2. Verify user has access to the parent channel
                              ├── 3. Fetch file from storage backend
                              │      ├── Local: read from disk
                              │      └── S3: GET via Barbacane S3 route
                              └── 4. Set Content-Type, Content-Disposition headers
```

- **Access control at the application level.** File access follows the same permission model as messages — if you can read the channel, you can download its attachments.
- **Content-Disposition.** Images and PDFs are served inline (`Content-Disposition: inline`). Other types are served as downloads (`Content-Disposition: attachment; filename="report.pdf"`).

### File constraints

| Constraint | Default | Configurable |
|-----------|---------|-------------|
| Max file size | 20 MB | Yes |
| Max files per message | 10 | Yes |
| Blocked extensions | Executables (`.exe`, `.bat`, `.sh`, `.msi`, `.cmd`, `.ps1`) | Yes (blocklist) |

```toml
[storage]
backend = "local"                      # "local" or "gateway" (S3 via Barbacane)
local_path = "/var/lib/burst/uploads"
max_file_size = "20MB"
max_files_per_message = 10
blocked_extensions = ["exe", "bat", "sh", "msi", "cmd", "ps1"]

[storage.gateway]                      # only when backend = "gateway"
base_url = "http://localhost:8080"     # Barbacane's public/internal URL
storage_path = "/storage"             # prefix for S3 dispatcher routes
```

- **Size validation happens early** — before streaming to storage. The `Content-Length` header is checked first. If absent, Burst streams with a byte counter and aborts if the limit is exceeded.
- **Content-type validation** uses both the declared MIME type and file extension.

### Image handling

For image attachments, Burst extracts metadata during upload:

- **Dimensions** (width × height) — read from the image header (JPEG, PNG, GIF, WebP). Enables the frontend to reserve space before the image loads, preventing layout shift.
- **No server-side thumbnails in v1.** The frontend handles responsive display. Server-side thumbnail generation is a future optimisation.

### File lifecycle

- **Deletion.** When a message is soft-deleted (per [ADR-007](007-data-model.md)), attachments are not immediately removed from storage. A background cleanup job periodically removes files whose parent messages have been soft-deleted for longer than a configurable retention period (default: 30 days).
- **Channel archival.** Archiving a channel does not delete files. Attachments remain accessible in read-only mode.
- **No versioning.** Files are immutable once uploaded. Editing a message does not modify its attachments.

### What we deliberately avoided

- **No CDN integration.** CDN can be added as a reverse proxy in front of download routes. Burst does not need to know about CDN configuration.
- **No virus scanning.** Out of scope for v1. Can be added as a pre-storage hook or via external service scanning the S3 bucket.
- **No server-side image processing.** No resizing, cropping, or format conversion.
- **No resumable uploads.** `multipart/form-data` is sufficient for the 20 MB default. Resumable protocols (tus) can be considered if the limit is raised significantly.

## Consequences

- **Burst holds zero S3 credentials.** Same pattern as auth — all cloud service credentials live in the Barbacane spec, not in the Burst application. This reduces Burst's security surface and centralises secret management.
- **Barbacane's S3 plugin gets stress-tested.** File uploads and downloads in a real messaging app exercise the S3 dispatcher with real workloads — binary content, large files, concurrent access. Bugs found here benefit all Barbacane users.
- **Switching storage backends is a configuration change.** Moving from local to S3 means updating the Burst config (`backend = "gateway"`) and adding S3 routes to the Barbacane spec. No Burst code change.
- **The gateway storage path adds one network hop.** File operations go Burst → Barbacane → S3 instead of Burst → S3 directly. This is the same trade-off accepted for auth. For large files, the extra hop adds latency — acceptable at team scale, and offset by the operational simplicity.
- **Local storage works without Barbacane's S3 plugin.** Simple deployments (single node, local disk) don't need S3 configuration at all. The storage trait ensures both paths are first-class.
- **No orphaned files.** Attachments are always tied to messages, created atomically, and cleaned up by a background job after soft deletion.
