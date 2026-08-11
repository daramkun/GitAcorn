# GitAcorn Patch Share API v1

GitAcorn can publish and import patches through a small self-hostable HTTP API. The service stores an opaque unified Git patch and metadata; it never receives repository credentials or modifies a Git repository.

## Transport and authentication

- Production endpoints must use HTTPS. `http://localhost`, `http://127.0.0.1`, and `http://[::1]` are accepted for local development.
- An endpoint contains no URL credentials, query, or fragment and may include a path prefix, for example `https://patch.example/git-acorn/`.
- A user-supplied token is sent as `Authorization: Bearer <token>`. GitAcorn keeps it only in the open dialog and does not persist it.
- Request and response bodies use UTF-8 JSON. Patch bodies are limited to 8 MiB.

## Publish a patch

`POST <endpoint>/v1/patches`

```json
{
  "schemaVersion": 1,
  "title": "Fix cache invalidation",
  "description": "Optional context",
  "repository": "team/project",
  "baseRevision": "main",
  "patch": "diff --git ...",
  "sha256": "lowercase SHA-256 of the UTF-8 patch bytes"
}
```

The service returns HTTP 2xx and:

```json
{
  "schemaVersion": 1,
  "patchId": "opaque-safe-id",
  "webUrl": "https://patch.example/p/opaque-safe-id",
  "sha256": "the submitted checksum",
  "expiresAt": "2026-09-01T00:00:00Z"
}
```

`webUrl` and `expiresAt` may be `null`. `patchId` contains only ASCII letters, digits, `_`, or `-` and is at most 128 characters. GitAcorn rejects a response whose checksum differs from the submitted patch.

## Fetch a patch

`GET <endpoint>/v1/patches/<patchId>` returns:

```json
{
  "schemaVersion": 1,
  "patchId": "opaque-safe-id",
  "title": "Fix cache invalidation",
  "description": "Optional context",
  "repository": "team/project",
  "baseRevision": "main",
  "patch": "diff --git ...",
  "sha256": "lowercase SHA-256 of the UTF-8 patch bytes",
  "createdAt": "2026-08-11T00:00:00Z",
  "expiresAt": "2026-09-01T00:00:00Z",
  "webUrl": "https://patch.example/p/opaque-safe-id"
}
```

Optional timestamp and URL fields may be `null`. GitAcorn verifies the checksum before exposing or validating the patch.

## Delete a patch

`DELETE <endpoint>/v1/patches/<patchId>` returns any HTTP 2xx response. Authorization policy and retention are controlled by the service.

## Error behavior

- `401` and `403` are treated as authentication failures.
- Other non-2xx responses are reported with the HTTP status only; response bodies are not surfaced to avoid leaking server details.
- A failure to publish, fetch, or delete never runs a Git command or changes the local repository.
- Imported content is previewed and checked with `git apply --check` before the separately confirmed apply operation.
