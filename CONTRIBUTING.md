# Contributing to Burst

Thanks for your interest in Burst. Bug reports, fixes, features, docs and questions are all welcome.

## Where to start

- **Issues labelled [`good first issue`](https://github.com/barbacane-dev/burst/labels/good%20first%20issue)** are self-contained and come with pointers to the code involved.
- **[`help wanted`](https://github.com/barbacane-dev/burst/labels/help%20wanted)** marks larger items we would like help with.
- **[Discussions](https://github.com/barbacane-dev/burst/discussions)** are for questions, ideas and show-and-tell.
- For anything larger than a bug fix, open an issue or a discussion first so we can agree on the approach before you write the code.

Comment on an issue to claim it, so two people don't work on the same thing.

## Development setup

Prerequisites: Rust (stable), Node.js 20+, Docker (or Podman), and [overmind](https://github.com/DarthSim/overmind) or hivemind to run the process set (`make install` installs overmind with Homebrew).

```bash
make all      # PostgreSQL + mock OIDC, gateway compile, then gateway + server + UI
make seed     # first time only: sample users
```

Open http://localhost:5173 and sign in as `alice` with any password. `make help` lists every target; the [README](README.md) describes the architecture and the individual processes.

The Makefile downloads the pinned Barbacane gateway binary into `.barbacane/`. To use a local Barbacane build instead, set `BARBACANE_BIN=../barbacane/target/release/barbacane`.

## Project layout

| Path | Contents |
|------|----------|
| `crates/burst` | Binary: CLI entry point, config loading |
| `crates/burst-core` | Domain types, events, pure logic (no I/O) |
| `crates/burst-server` | Axum handlers, database (sqlx), WebSocket, file storage |
| `ui/` | React 19, Vite, Tailwind, TanStack Query |
| `specs/burst-api.yaml` | The OpenAPI spec: the API contract and the gateway configuration |
| `migrations/` | PostgreSQL migrations |
| `adr/` | Architecture Decision Records |
| `docs/` | User, deployment and admin documentation (mdBook) |
| `deploy/helm/` | Helm chart |

The [ADRs](adr/) explain why Burst is built the way it is. Read the ones an issue references before starting on it. A change that contradicts an ADR needs a discussion first, and possibly a new ADR.

## Before opening a pull request

Run the same checks CI runs:

```bash
make check                          # cargo fmt, clippy -D warnings, cargo test (needs PostgreSQL from `make services`)
make lint-spec                      # OpenAPI lint; a hard gate in CI
cd ui && npm run lint && npm test && npm run build
```

- After changing `specs/burst-api.yaml`, run `make gateway-compile`.
- Integration tests live in `crates/burst-server/tests/` and use `#[sqlx::test]` against a real database.
- UI changes: include a screenshot or short recording in the PR.
- User-visible changes: update the relevant page in `docs/`.

Rust conventions: no `unwrap()` or `panic!()` in production code, `thiserror` for library errors and `anyhow` in the binary, and errors returned as RFC 9457 problem details with `urn:burst:error:<type>` URNs.

## Commits and pull requests

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/). Releases and the changelog are generated from them:

```
fix(ui): show custom emojis as images in reactions
feat(search): filter messages by author
```

Common types: `feat`, `fix`, `docs`, `refactor`, `test`, `ci`, `chore`. A breaking change adds `!` after the type.

Sign off every commit to certify the [Developer Certificate of Origin](https://developercertificate.org/), which states that you wrote the change or otherwise have the right to submit it:

```bash
git commit --signoff
```

Keep each pull request to one logical change. A maintainer reviews every PR, and CI must pass before merge.

## Reporting security issues

Do not open a public issue for a vulnerability. See [SECURITY.md](.github/SECURITY.md).

## Code of conduct

Everyone taking part is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

Burst is licensed under the [Apache License 2.0](LICENSE). By contributing, you agree that your contributions are licensed under the same terms.
