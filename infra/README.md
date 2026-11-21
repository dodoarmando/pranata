# infra

The `pranata` Ansible role provisions one Installation on a fresh Debian 13 host. One run sets up the host, starts the four containers and, the first time only, prints the first Administrator's Access Link.

What it sets up:

- **Host**: unattended-upgrades, key-only SSH without root login, and an nftables firewall that opens only 22, 80 and 443 (TCP, and UDP 443 for HTTP/3).
- **Containers**: rootless Podman quadlets under a dedicated `pranata` user: `caddy`, `web`, `api` and `postgres` on the `pranata` network. Data lives in the named volumes `pranata-postgres`, `pranata-files` and `pranata-caddy-data`.
- **Ports**: only Caddy is reachable. It gets ports 80 and 443 from `caddy.socket` (systemd socket activation), not from a published port. A port published from a rootless Podman network would show every client as the network's gateway, and the API throttles by client address.
- **Secrets**: the PostgreSQL password from Ansible Vault, stored as the Podman secrets `pranata-postgres-password` and `pranata-database-url`.
- **Bootstrap**: `pranata migrate` on every run. On the first run only, `pranata village set-code`, `pranata seed` on the demo, and `pranata user create --admin`.

## The private ops repo

The inventory, the pinned release and the Vault secrets live in a private ops repo, never here. The variables are listed in [`roles/pranata/meta/argument_specs.yml`](roles/pranata/meta/argument_specs.yml), and the role checks them before it changes anything. A host's variables look like this:

```yaml
pranata_version: 1.0.0
pranata_fallback_domain: example.vendor.id
pranata_domain: example.desa.id # leave out until the Village's domain is ready
pranata_village_code: "1101012001"
pranata_administrator_username: kades
pranata_vendor_ssh_keys: ["ssh-ed25519 AAAA… vendor"]
pranata_postgres_password: !vault |
  …
```

Install the collections, then run the playbook:

```sh
ansible-galaxy collection install -r infra/requirements.yml
ansible-playbook -i inventory --ask-vault-pass infra/provision.yml
```

Before the first run, point DNS at the host: the fallback domain, plus the `.desa.id` domain and its `www` once `pranata_domain` is set. Caddy needs that DNS to get certificates.

The first run connects as root, with `-u root`, and needs `sudo` on the host, as Debian's cloud images have. It creates `pranata_vendor_user` with the given SSH keys and passwordless sudo, then turns off root login. Later runs connect as that user.

If the bootstrap fails partway, a rerun repeats it, and `pranata seed` and `pranata user create` refuse what already ran: finish those steps with the CLI, then create the marker file `~pranata/.local/state/pranata/bootstrapped`.

The `pranata-api` and `pranata-web` images must be public on GHCR, since the host pulls them without logging in.

## The demo Installation

The demo is an ordinary host with `pranata_seed_demo: true` and no `pranata_domain`, so it serves the synthetic seed data on its fallback subdomain. To provision it from scratch, rebuild the VPS and run the playbook again.

## Lint

`just check-infra` runs `ansible-lint` with the tools pinned in `requirements.txt`. CI runs it in the `infra` job.
