# Releasing

Releases are cut by release-please from Conventional Commits on `main`. One
version covers the whole repository. Nothing is deployed from CI (D-60); a
release only publishes images.

## Flow

1. Merging to `main` makes release-please open or update a release pull request
   titled `chore(release): X.Y.Z`. It bumps `.release-please-manifest.json`,
   `apps/api/Cargo.toml`, the `pranata` entry in `Cargo.lock`, the root and
   `apps/web` `package.json` files, and writes `CHANGELOG.md`.
2. Merging that pull request creates the tag `vX.Y.Z` and a GitHub release.
3. The same workflow then builds both images and pushes
   `ghcr.io/dodoarmando/pranata-api` and `pranata-web`, tagged `X.Y.Z` and
   `latest`.

`just check-versions` (run by the `lint` job) fails when those files disagree
with the manifest, so `--version`, `/api/healthz` and the image tag always match.

## The first release is pinned to 0.1.0

`release-please-config.json` carries `"release-as": "0.1.0"` on the package
entry. The manifest already says `0.1.0` while no tag exists yet, so without the
pin release-please would compute the next version from the commits and publish
0.2.0 or 0.1.1 as the first release. **Remove `release-as` in the pull request
that follows the 0.1.0 release.** If it stays, every later release pull request
is stuck at 0.1.0.

## Maintainer setup (one time)

Pull requests opened with the default `GITHUB_TOKEN` do not trigger other
workflows, so the release pull request would never get its required checks.
Create a token and store it as the repository secret `RELEASE_PLEASE_TOKEN`:

- Fine-grained personal access token, resource owner `dodoarmando`, repository
  access limited to `dodoarmando/pranata`, with repository permissions
  Contents: read and write, Pull requests: read and write, Issues: read and
  write (release-please manages labels), and Metadata: read. Or a GitHub App
  installed on the repository with the same permissions, with a step that mints
  its token.
- Without the secret the workflow falls back to `GITHUB_TOKEN`. That also needs
  Settings, Actions, General, "Allow GitHub Actions to create and approve pull
  requests" turned on, and the release pull request then needs its checks
  triggered by hand (push an empty commit to its branch from a real account).

The first push of the two ghcr packages uses `GITHUB_TOKEN` with `packages:
write`. The images carry the `org.opencontainers.image.source` label so ghcr
links them to the repository. If a push is refused, link the package to the
repository in its settings.

## Bot commits and the commit rules

- Renovate commits are signed off with the bot identity and use the
  `build(deps)` type and scope, so DCO and commitlint accept them.
- release-please's release commit has no sign-off. `commitlint.config.ts`
  skips exactly the title `chore(release): X.Y.Z`. Anyone can write that title,
  so reviewers must not approve a human commit that uses it; real release
  commits come only from the release pull request.
