# One Installation per Village

Pranata is a product for many villages, but each Village runs its own Installation (its own containers and PostgreSQL database) instead of sharing one multi-tenant deployment. Population records (NIK, family cards) are personal data under Indonesia's Personal Data Protection Law (UU 27/2022), and physical isolation per Village is the simplest way to keep one Village's data out of another's reach and to let a Village self-host under the AGPL.

## Considered Options

- **Multi-tenant SaaS** (one deployment, `village_id` on every row): cheaper to operate, but one missed filter leaks personal data across villages, and self-hosting would still need a single-tenant mode.
- **Single-tenant now, multi-tenant-ready code**: rejected as speculative complexity; no tenant column or tenant routing exists in the code.

## Consequences

Updates, backups, and monitoring are per Installation, so releases must ship as versioned container images that an Installation can pull and migrate on its own.
