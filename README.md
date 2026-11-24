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

## Local development

You also need `sqlx-cli` for `just db-migrate`:

```sh
cargo install sqlx-cli --locked --no-default-features --features rustls,postgres
```

1. Start PostgreSQL in a Podman container on `127.0.0.1:55432`, then apply the migrations:

   ```sh
   just db-up
   just db-migrate
   ```

2. Run the API (`127.0.0.1:8080`) and the web dev server (`http://localhost:3000`) together. Leave it running and use a second terminal for the next steps.

   ```sh
   just dev
   ```

   The web dev server forwards `/api` to the API and serves uploaded photos at `/media`, so no Caddy is needed. The dev-only settings are in `infra/dev/env.sh`. A variable you export yourself takes precedence.

3. Create the Village from a 10-digit code in the bundled region dataset (`apps/api/data/region-codes.csv.gz`). Any code works, for example:

   ```sh
   just dev-village-init 14.05.03.2002
   ```

4. Create the first Administrator. The command prints an Access Link, valid for 24 hours and usable once:

   ```sh
   just dev-create-admin admin "Admin Desa"
   ```

5. Open the Access Link in a browser and set a password (10 to 128 characters). Then sign in at `/masuk`. Edit the overview and the hero photo at **Desa → Profil desa**. After you save, `http://localhost:3000/` shows the change.

To start over with an empty database, remove the container and the uploaded files, then repeat from step 1:

```sh
podman rm -f pranata-db
rm -rf target/dev-media
```

`just` lists the other recipes. `just test` runs the tests and needs the database from step 1. `just e2e` runs the end-to-end tests against the production images behind Caddy.

## Contributing

Commits follow [Conventional Commits](https://www.conventionalcommits.org) and must carry a DCO sign-off (`git commit -s`). Hooks installed by lefthook enforce this locally; `just check-commits` runs the same check over a range.

## Licence

AGPL-3.0. See [LICENSE](LICENSE).
