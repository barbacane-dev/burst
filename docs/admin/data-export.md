# Data Export

An admin can export messages and files as a zip archive, for compliance or to move to another system.

## Scope

- **Whole instance**: every channel, including private channels and direct messages, and every user.
- **One channel**: that channel, and the users who are its members or wrote in it.

Deleted messages and their files are left out.

## Exporting from the Admin Panel

Open **Administration → Exports**, choose **Whole instance** or a channel, and click **Start export**. The export is built in the background; the list shows it as **Queued**, then **Building**, then **Ready**. Click **Download** to save the archive, and **Delete** to remove it once you no longer need it.

One export runs at a time. If the server building an export restarts, the export stops making progress; after ten minutes it is shown as **Failed** with the reason `interrupted`, and a new one can be started.

The browser holds a downloaded archive in memory before saving it. For a very large instance, download with the API instead:

```bash
curl -H "Authorization: Bearer $TOKEN" -o export.zip \
  https://burst.example.com/api/admin/exports/<export id>/download
```

## Archive Layout

```text
export.json                           format, scope, who requested it, counts
users.json                            the users the export names
channels/<channel id>/channel.json    the channel and its members
channels/<channel id>/messages.jsonl  one message per line, oldest first
files/<attachment id>/<file name>     attachments as uploaded
```

`export.json` has `"format": "burst-export/1"`. Each line of `messages.jsonl` is a message with its author, thread, edit time, attachments (with their `path` in the archive) and reactions. An attachment whose file was missing from storage has no `path` and is listed under `missingFiles` in `export.json`.

## Storage and Audit

Archives are kept in the configured file storage under `exports/`, until deleted. Requesting, downloading and deleting an export are recorded in the [audit log](audit-log.md) as `export.requested`, `export.downloaded` and `export.deleted`.

## API

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/admin/exports` | Start an export. Body `{ "channelId": "ch_..." }` for one channel, `{}` for the instance. |
| `GET` | `/api/admin/exports` | The 50 most recent exports. |
| `GET` | `/api/admin/exports/{id}` | One export's status. |
| `GET` | `/api/admin/exports/{id}/download` | The archive. |
| `DELETE` | `/api/admin/exports/{id}` | Delete a finished export and its archive. |
