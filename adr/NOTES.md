# ADR Notes

Observations and ideas captured during ADR discussions. These are not decisions — they are inputs for future ADRs or implementation work.

## UI/UX Layer

- **Thread escalation to channel:** Slack-style "continue in channel" behaviour. The data model (ADR-007) supports this natively — it's a UI action that creates/selects a channel and posts a linking message. No schema change needed.

## Barbacane Contributions

- **ldap-auth plugin:** Already in Barbacane roadmap (P2). Burst's LDAP requirement (ADR-006) is a strong reason to prioritise it. Building it as a Barbacane plugin benefits both projects.
- **websocket dispatcher plugin:** Also in Barbacane roadmap (P2). Could simplify Burst's WebSocket proxying topology.
- **Expose `components/schemas` on `ApiSpec`:** `barbacane-compiler` resolves `$ref`s inline into operations but doesn't expose component schemas as a top-level collection. Burst's spec-sync tests need to compare Rust structs against schema definitions — currently done with raw `serde_yaml` parsing. If `ApiSpec` exposed `schemas: BTreeMap<String, serde_json::Value>`, downstream projects could validate their response types against the spec using Barbacane's own parser (with `$ref` resolution, depth/complexity checks, etc.).
