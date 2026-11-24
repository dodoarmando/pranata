# One Installation per Village, on its own VPS

Pranata is a product for many villages, but each Village runs its own Installation: its own VPS, containers and PostgreSQL database. The maintainer operates Installations as a paid installation service and each Village pays for its own hosting; anyone else may self-host the same images under the AGPL.

Physical isolation is the simplest way to keep one Village's data out of another's reach, which matters once population records (personal data under UU 27/2022) arrive in R2. It also lets one Village's load spike or outage stay its own.

## Considered Options

- **Multi-tenant SaaS** (one deployment, `village_id` on every row): cheapest to run, but one missed filter leaks personal data across villages, and self-hosting would still need a single-tenant mode.
- **Several Installations on one shared server**: cheaper per village, but a shared machine is a shared risk and needs per-Installation resource limits. The images do not prevent it later.
- **Single-tenant now, multi-tenant-ready code**: rejected as speculative complexity; no tenant column or tenant routing exists.

## Consequences

Updates, backups and monitoring are per Installation, so releases ship as versioned images that an Installation pulls and migrates on its own, and one command must be able to update one or all Installations. The whole stack must fit a small VPS (2 GB RAM, 1–2 vCPU).
