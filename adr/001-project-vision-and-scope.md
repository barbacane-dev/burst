# ADR-001: Project Vision & Scope

**Status:** Accepted, amended by [ADR-016](016-competitive-position-reassessment.md) (2026-09-24): the "clear gap" in Context does not hold, since Zulip is fully open with ungated SSO. The five principles stand; the differentiator is restated as a familiar messaging model in one binary plus a database.
**Date:** 2026-03-06

## Context

Barbacane needs a reliable, focused team messaging tool. The two main open-source options — Rocket.Chat and Mattermost — have progressively drifted away from their core messaging mission:

- **Platform creep:** Both products have expanded into marketplace apps, omnichannel customer support, AI assistants, video conferencing wrappers, and CRM-like features, diluting the messaging experience.
- **OSS erosion:** Key features (LDAP/SAML auth, compliance exports, advanced permissions, high-availability) have been moved behind proprietary Enterprise licenses, making the open-source editions increasingly inadequate for real-world team use.
- **Complexity tax:** The bloated feature set leads to heavier resource consumption, slower release cycles, and a steeper learning curve for administrators and users alike.

There is a clear gap for a messaging tool that does one thing well: real-time team communication.

## Decision

We will build **Burst**, an open-source team messaging application with the following guiding principles:

1. **Messaging first, messaging only.** Burst is a messaging tool — not a platform. We will not build or bundle unrelated capabilities (CRM, helpdesk, video conferencing, app marketplace). Integrations with external tools are welcome, but the core product stays focused.

2. **Fully open-source.** All features ship under a single open-source license. There is no "Enterprise Edition" gating. If a feature is built, it is available to everyone.

3. **Simple to operate.** A small team should be able to deploy, configure, and maintain Burst without dedicated infrastructure staff. Sensible defaults, minimal dependencies, clear documentation.

4. **Lightweight and fast.** Burst should be resource-efficient. A small-to-medium organisation should be able to run it comfortably on modest hardware.

5. **Opinionated but extensible.** We make strong default choices to keep the product coherent, but expose well-defined APIs and webhooks for teams that need to integrate with their existing tooling.

## Consequences

- We will say **no** to feature requests that fall outside core messaging, even if they seem convenient. Scope discipline is a feature.
- We accept that Burst will not compete with Rocket.Chat or Mattermost on breadth of features. We compete on focus, reliability, and operational simplicity.
- The fully open-source model means we need a sustainable funding approach that does not rely on feature gating (e.g., support contracts, hosted offering, sponsorships).
- Keeping the stack lightweight constrains our technology choices — we will favour proven, efficient technologies over trendy ones.
