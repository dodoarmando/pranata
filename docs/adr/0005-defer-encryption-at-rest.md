# Defer disk encryption to R2; encrypt backups from R1

R1 runs without LUKS/Clevis/Tang. Backups are encrypted (restic) from the first release. Before population data arrives in R2, a new decision chooses between disk encryption and column-level encryption of sensitive fields such as NIK.

On a per-village VPS, network-bound disk unlocking needs a separate, always-on Tang server; if it is unreachable when the VPS reboots, the village's site stays down until someone unlocks it by hand. The VPS provider also controls the hypervisor, so disk encryption protects mainly against leaked snapshots or discarded disks. R1 holds almost no personal data (staff accounts and the public names of officials), so the cost outweighs the benefit now.

## Considered Options

- **LUKS2 + Clevis/Tang from R1** (the archive's choice): strongest at-rest story, but adds an extra server and an availability risk for little protected data.
- **LUKS2 with a passphrase typed at boot**: every reboot needs a human.

## Consequences

R2's design must include the encryption decision as a blocking item. Backup encryption keys are stored outside the VPS.
