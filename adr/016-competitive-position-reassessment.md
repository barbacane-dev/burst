# ADR-016: Competitive Position Reassessment

**Status:** Accepted
**Date:** 2026-09-24

## Context

[ADR-001](001-project-vision-and-scope.md) named Rocket.Chat and Mattermost, described their drift away from messaging and their movement of core features behind Enterprise licenses, and concluded that there is a clear gap for a tool that does one thing well. [ADR-002](002-core-feature-set.md) built a feature set on that reading and recorded, as a consequence, that ungated LDAP/SSO and audit logs are a differentiator.

Six months on, two of those claims have hardened and one does not hold.

**Hardened: features moving behind a license.** Rocket.Chat now caps its Starter plan at a small user count, replacing the unlimited community edition; high-availability clustering, message auditing and data loss prevention require Enterprise. Mattermost replaced the unlimited Team Edition with a seat-limited free tier positioned for evaluation. Both moved further in the direction ADR-001 described.

**Hardened: platform creep.** Mattermost pairs chat with workflow automation, voice and screen sharing. The newer self-hosted entrants bundle more, not less: Huly combines chat with project management, documents and a virtual office; Colanode combines chat with pages, databases and file management. Nothing in the category moved toward focus.

**Does not hold: the gap.** Zulip is Apache 2.0 end to end with no enterprise tier, and SAML, LDAP, Active Directory federation, SCIM provisioning and granular RBAC are all available in the self-hosted edition at no cost. A foundation was formed in 2026 to steward it. ADR-001 principle 2, "fully open-source, no Enterprise Edition gating", and most of ADR-002's Administration section describe a product that already exists and is mature. ADR-001 does not mention Zulip, and that omission is the document's defect rather than a change in the market.

Two things Zulip does not do:

- **Run small.** It is Django, Tornado, RabbitMQ, Redis, Memcached, PostgreSQL and nginx, and its own documentation asks for 4 GB of RAM at 25 daily active users and 8 GB at 100.
- **Look like Slack.** Its topic-based threading is a distinct model. A team leaving Slack or Teams is not choosing between two equivalent products, and the model is adopted enthusiastically or abandoned quickly.

## Decision

**1. The differentiator is restated.** It is not "fully open source", which Zulip reached first. It is *familiar, and one binary plus a database*. Stated against each alternative:

| Against | The claim |
|---|---|
| Rocket.Chat, Mattermost | No seat cap, no SSO or audit log behind a license |
| Zulip | A conventional channel-and-thread model, and an order of magnitude less to run |

**2. ADR-001's five principles stand unchanged.** Principle 2 in particular stays, because it is a promise to the people running Burst, not a claim to be the only one making it.

**3. ADR-002's consequence that ungated LDAP/SSO and audit logs are "a differentiator" is withdrawn.** They are table stakes. Zulip offers all of them free, and Burst does not ship LDAP at v1.0 at all. Keeping them ungated remains correct; presenting them as a reason to choose Burst does not.

**4. v1.0 must publish a measured footprint comparison.** Resident memory, process count, installed packages and cold start for standalone Burst against a Zulip install at the same number of daily active users, on the same hardware. A claim to be lighter is not credible as an assertion, and it is the one claim that does not depend on taste.

## Consequences

- **The build order does not change.** Milestone 11 (standalone tier) and Milestone 12 (local accounts) are precisely the work that makes the restated claim true. The reassessment changes how the project is justified, not what it does next.
- **The comparison becomes a release gate**, recorded in Milestone 13. Publishing it is the deliverable, whatever it shows.
- **The footprint result can falsify the premise.** If standalone Burst cannot serve 25 daily active users in a small fraction of the 4 GB Zulip asks for, the restated differentiator is one claim short and the project should be reconsidered rather than shipped on the remaining one. That threshold is a hypothesis to test at M13, not a measurement.
- **The user experience argument carries real weight but cannot carry the position alone.** That Zulip is not Slack-shaped is true and is why a team can reject it without rejecting open source. It is also a judgement an evaluator may not share, which is why the footprint number matters: it is the leg that holds regardless of preference.
- **LDAP deferred to Milestone 14 now reads as a gap rather than a differentiator held back.** Against Zulip it is a missing feature. The OIDC bridge remains an adequate answer for v1.0, but the framing in ADR-002 should not be relied on.
