# Security Policy

## Scope

`ringmaster.rs` handles sensitive personal health data and OAuth credentials. Even though the app is local-first and single-user, security issues still matter.

## Reporting

If you discover a security issue, do not open a public issue with exploit details. Report it privately to the maintainer first.

Until a dedicated channel exists, use the repository contact details or GitHub security reporting if enabled.

## Current security posture

- local-first SQLite storage
- explicit auth/sync/store boundaries
- no token logging
- demo mode with deterministic non-user data

## Guardrails for contributors

- never log access tokens, refresh tokens, or raw secrets
- do not store user tokens in plaintext config files
- preserve secret storage behind an OS keyring or clearly isolated equivalent seam
- treat raw Oura payloads as sensitive data
- keep webhook/security-sensitive seams isolated even if webhook delivery is deferred
