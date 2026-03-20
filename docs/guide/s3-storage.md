## S3 Storage via Barbacane Sidecar

Burst supports two file storage backends: **local filesystem** (default) and
**S3-compatible object storage** via a Barbacane S3 sidecar. You choose the
backend with a single configuration option.

## Local vs S3

| | Local | S3 |
|---|---|---|
| Default | Yes | No |
| Config value | `storage.backend = "local"` | `storage.backend = "gateway"` |
| Multi-node | Not supported (files are on one disk) | Required for horizontal scaling |
| S3 credentials in Burst | N/A | **Never** — the sidecar handles them |
| Good for | Development, single-node production | Multi-node production |

Use local storage when you run a single Burst instance and want the simplest
setup. Switch to S3 when you need shared file access across multiple nodes.

## How It Works

Burst never talks to S3 directly. Instead, a dedicated **Barbacane S3 sidecar**
sits between Burst and your S3 provider. The sidecar authenticates Burst requests
with an API key, then dispatches them to S3 using its own credentials.

```
User --> Public Gateway (:8080) --> Burst server (:3000)
                                        |
                                        v
                                   S3 Sidecar (:8081) --> S3 (MinIO / RustFS / AWS)
```

This design means:

- Burst has **zero S3 credentials** in its configuration.
- You can swap S3 providers by reconfiguring the sidecar, not the application.
- The sidecar runs on `localhost` (or a private network), so it is never exposed
  to end users.

## Burst Configuration

Point Burst at the S3 sidecar (not the public gateway):

```toml
[storage]
backend = "gateway"
gateway_url = "http://127.0.0.1:8081"
gateway_api_key = "a-long-random-secret"
```

Or with environment variables:

```bash
BURST_STORAGE_BACKEND=gateway
BURST_STORAGE_GATEWAY_URL=http://127.0.0.1:8081
BURST_STORAGE_GATEWAY_API_KEY=a-long-random-secret
```

The `gateway_url` must point to the **S3 sidecar** (port 8081 by default),
**not** the public gateway (port 8080). The public gateway handles user traffic;
the sidecar handles file storage.

## Setting Up an S3 Provider

### RustFS

[RustFS](https://github.com/rustfs/rustfs) is a lightweight S3-compatible
server written in Rust, well suited for self-hosted Burst deployments.

```bash
rustfs server --address 0.0.0.0:9000 --console-address 0.0.0.0:9001
```

Default credentials are `minioadmin` / `minioadmin`. Create a bucket for Burst:

```bash
aws --endpoint-url http://127.0.0.1:9000 s3 mb s3://burst-files
```

### MinIO

[MinIO](https://min.io/) is a popular S3-compatible server.

```bash
minio server /data --address 0.0.0.0:9000 --console-address 0.0.0.0:9001
```

Create a bucket the same way:

```bash
mc alias set local http://127.0.0.1:9000 minioadmin minioadmin
mc mb local/burst-files
```

### AWS S3

No local server needed. Create a bucket in your AWS account and note the region.

## Sidecar Configuration

The S3 sidecar is a Barbacane instance running the `s3` and `apikey-auth` plugins.
Its spec lives at `specs/burst-s3.yaml`. Configure it with environment variables:

```bash
BURST_S3_REGION=us-east-1
BURST_S3_BUCKET=burst-files
BURST_S3_ACCESS_KEY_ID=minioadmin
BURST_S3_SECRET_ACCESS_KEY=minioadmin
BURST_S3_ENDPOINT=http://127.0.0.1:9000
BURST_S3_API_KEY=a-long-random-secret
```

The `BURST_S3_API_KEY` must match the `gateway_api_key` in your Burst config.
This shared secret authenticates Burst to the sidecar.

### force_path_style

RustFS and MinIO require **path-style** S3 URLs (`http://host:9000/bucket/key`)
instead of virtual-hosted-style (`http://bucket.host:9000/key`). The sidecar
spec enables `force_path_style: true` in the S3 dispatcher config by default.

If you use AWS S3 with virtual-hosted-style URLs, set `force_path_style: false`
in the spec and recompile:

```bash
make gateway-compile
```

## Two Barbacane Instances

A full Burst deployment with S3 storage runs two separate Barbacane processes:

| Instance | Port | Role | Exposed to users |
|----------|------|------|------------------|
| Public gateway | 8080 | REST API + WebSocket proxy | Yes |
| S3 sidecar | 8081 | File storage proxy | No (internal only) |

They are independent processes with separate compiled artifacts (`burst-api.bca`
and `burst-s3.bca`). You start them separately:

```bash
barbacane run burst-api.bca --listen 0.0.0.0:8080
barbacane run burst-s3.bca --listen 0.0.0.0:8081
```

## File Lifecycle

1. A user uploads a file through the public gateway.
2. The gateway proxies the multipart request to Burst.
3. Burst stores metadata in PostgreSQL and streams the file to the S3 sidecar.
4. The sidecar writes the file to your S3 provider.
5. On download, the reverse path applies: Burst reads from the sidecar and
   streams the response back through the gateway.

## Soft Deletes and Cleanup

When a message with attachments is deleted, Burst soft-deletes the records.
A background task periodically removes the actual S3 objects after a retention
period. You control this with:

```toml
[storage]
cleanup_interval_secs = 3600      # how often the cleanup task runs
cleanup_retention_days = 30       # how long soft-deleted files are kept
```

## Troubleshooting

- **403 from the sidecar** — verify that `BURST_S3_API_KEY` matches
  `storage.gateway_api_key` in your Burst config.
- **Connection refused on port 8081** — make sure the sidecar process is running
  and listening on the expected address.
- **"bucket does not exist"** — create the bucket before starting the sidecar.
  The sidecar does not create buckets automatically.
- **Signature errors with RustFS/MinIO** — confirm `force_path_style` is enabled
  in the sidecar spec.
