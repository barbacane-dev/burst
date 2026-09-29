# Security Policy

## Reporting a vulnerability

**Do not open a public GitHub issue for a security vulnerability.**

Report it privately, either through [GitHub's private vulnerability reporting](https://github.com/barbacane-dev/burst/security/advisories/new) or by email to **security@barbacane.dev**.

Please include:

- A description of the vulnerability and its impact
- Steps to reproduce it
- The affected version or commit
- A suggested fix, if you have one

## Response

- **Acknowledgement:** within 48 hours
- **Initial assessment:** within 7 days
- **Fix:** critical vulnerabilities are targeted within 30 days

We credit reporters in the security advisory unless you prefer to remain anonymous, and ask that you allow reasonable time for a fix before public disclosure.

## Supported versions

Security fixes are released for the latest minor version.

| Version | Supported |
|---------|-----------|
| 0.2.x   | Yes       |
| < 0.2   | No        |

## Scope

In scope: the Burst server and UI in this repository, the Barbacane gateway configuration it ships (`specs/`, `barbacane*.yaml`), and the Docker images and Helm chart built from it.

Out of scope: vulnerabilities in the Barbacane gateway itself (report those to the [Barbacane project](https://github.com/barbacane-dev/barbacane/security)), in other third-party dependencies, and issues that require physical access to the server or social engineering.
