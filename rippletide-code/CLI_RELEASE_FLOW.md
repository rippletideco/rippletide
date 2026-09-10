# Coding Agent CLI Release Flow

This is the intended release policy for `rippletide-code`.

## Branches and Channels

- `staging` drives the npm dist-tag `internal`
- `main` drives the npm dist-tag `latest`

## Maintainer Flow

1. Move the CLI changes onto `staging`.
2. Push `staging`.
3. Run the **Release Coding Agent** workflow manually with:
   - `ref=staging`
   - `version=<x.y.z-internal.n>`
4. Test the published internal build:
   - `npx rippletide-code@internal`
5. If the internal build is good, merge or fast-forward `staging` into `main`.
6. Publish the stable CLI by pushing a tag on `main`:
   - `v<x.y.z>`

## Examples

Internal test release:

- ref: `staging`
- version: `0.5.47-internal.1`

Stable release:

- tag: `v0.5.47`

## Guardrails

- Internal releases are only allowed from `staging`.
- Stable releases are only allowed from a tag that points to a commit reachable from `main`.
- `main` by itself does not publish the CLI.

## Why

This keeps three states separate:

- backend staging work
- CLI internal testing
- public npm `latest`

That separation avoids the earlier failure mode where a staging backend could be tested with an older published CLI.
