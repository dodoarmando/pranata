# AGPLv3 with Developer Certificate of Origin sign-off

Pranata is licensed AGPLv3 so that anyone running a modified Pranata for villages must share their changes. Every commit carries a `Signed-off-by` trailer (`git commit -s`) certifying the Developer Certificate of Origin, which records each contributor's right to submit their code without the friction of a Contributor License Agreement.

## Considered Options

- **CLA**: keeps the option to relicense, but deters outside contributors.
- **No sign-off**: simplest, but leaves the provenance of contributions unrecorded.

## Consequences

The project gives up the option to relicense contributions later. CI rejects commits without sign-off.
