# ADR Notes

Observations and ideas captured during ADR discussions. These are not decisions — they are inputs for future ADRs or implementation work.

## UI/UX Layer

- **Thread escalation to channel:** Slack-style "continue in channel" behaviour. The data model (ADR-007) supports this natively — it's a UI action that creates/selects a channel and posts a linking message. No schema change needed.

## Barbacane Contributions

- **ldap-auth plugin:** Already in Barbacane roadmap (P2). Burst's LDAP requirement (ADR-006) is a strong reason to prioritise it. Building it as a Barbacane plugin benefits both projects.
- **websocket dispatcher plugin:** Also in Barbacane roadmap (P2). Could simplify Burst's WebSocket proxying topology.

## Deferred Decisions

- **Licensing ADR:** Not needed while the repo is private. Write it before going public.
