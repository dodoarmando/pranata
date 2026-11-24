# Pranata

Pranata is a village website and administration product for Indonesian villages: a public site for residents and an admin area where village staff manage records and site content. Each village runs its own installation.

The domain glossary is in [CONTEXT.md](CONTEXT.md). Decisions and designs live in [docs/](docs/).

## Layout

- `apps/api`: Rust API.
- `apps/web`: TanStack Start web app.
- `docs/`: architecture decision records, specs and plans.
- `tools/`: repository tooling.

## Requirements

- Rust 1.100.0 (pinned by `rust-toolchain.toml`)
- [Bun](https://bun.sh)
- [just](https://just.systems)
- [Podman](https://podman.io)

## Getting started

```sh
bun install   # installs tooling and sets up git hooks
just          # lists the available recipes
```

## Contributing

Commits follow [Conventional Commits](https://www.conventionalcommits.org) and must carry a DCO sign-off (`git commit -s`). Hooks installed by lefthook enforce this locally; `just check-commits` runs the same check over a range.

## Licence

AGPL-3.0. See [LICENSE](LICENSE).
