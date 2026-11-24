# Pranata infrastructure

This guide takes you from a fresh VPS and a domain to a running Pranata
Installation with its first Administrator. One Installation serves one village
and runs on its own VPS (ADR 0001). Ansible, run from your own computer,
provisions it. Nothing is deployed from CI, and CI holds no SSH keys to village
servers.

Placeholders in angle brackets, such as `<your-domain>`, are yours to replace.

## Before you start

### What to buy or prepare

| What         | Requirement                                                                                  |
| ------------ | -------------------------------------------------------------------------------------------- |
| VPS          | Debian 13, amd64, 2 vCPU, 2 GB RAM, at least 40 GB disk. Prefer a provider in Indonesia.      |
| Domain       | A domain or subdomain with an `A` record (and an `AAAA` record if the VPS has IPv6) pointing at the VPS. |
| Bucket       | An S3-compatible bucket for backups, with an access key limited to that bucket. Prefer a provider in Indonesia. |
| Ping URLs    | Optional. Up to three Healthchecks.io-compatible URLs (`backup`, `restore`, `disk`), see [Monitoring](#monitoring). |
| Your machine | Linux or macOS with `just`, Python 3 (with the `venv` module), `git`, `ssh` and `openssl`. |

The DNS record must resolve to the VPS before you provision, or the
certificate cannot be issued (see [Troubleshooting](#troubleshooting)).
Ports 80 and 443 must be reachable from the internet; the role also opens SSH.

Provisioning turns off SSH root login and SSH password login, and it refuses to
do so unless you connect as a non-root user that has your key. Read the next
section before you run it.

### A non-root sudo user on the VPS

Ansible connects as an ordinary user and uses `sudo` without a password prompt.
Most providers give you only `root` on a new VPS. Create the user once, from
your computer:

```sh
ssh root@<vps-ip>
```

then on the VPS, as root (replace `operator` if you prefer another name):

```sh
apt-get update && apt-get install -y sudo python3
adduser --disabled-password --gecos "" operator
usermod -aG sudo operator
echo 'operator ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/90-operator
chmod 0440 /etc/sudoers.d/90-operator
visudo -cf /etc/sudoers.d/90-operator
install -d -m 0700 -o operator -g operator /home/operator/.ssh
cp /root/.ssh/authorized_keys /home/operator/.ssh/authorized_keys
chown operator:operator /home/operator/.ssh/authorized_keys
chmod 0600 /home/operator/.ssh/authorized_keys
```

This copies root's authorized keys. If your provider set a root password
instead of a key, run `ssh-copy-id operator@<vps-ip>` from your computer after
`adduser` without `--disabled-password`, or paste your public key into
`/home/operator/.ssh/authorized_keys` by hand.

Before you leave the root session, open a second terminal and check that this
works:

```sh
ssh operator@<vps-ip> sudo -n true
```

If it prints nothing and exits 0, you are ready. If it asks for a password or
fails, fix it first: after provisioning, only key logins as a non-root user
work.

## Prepare your machine

1. Clone the repository and enter it:

   ```sh
   git clone https://github.com/dodoarmando/pranata.git
   cd pranata
   ```

2. Check the infrastructure tooling once:

   ```sh
   just check-infra
   ```

   The first run creates a Python virtual environment in `target/infra` with the
   pinned Ansible and its collections. It also builds a small Debian 13 image
   with Podman to dry-run the service units, so it needs Podman on your machine.
   If you do not have Podman, skip this step: `just provision` creates the same
   environment on its own.

3. Create your private inventory outside the repository, for example:

   ```sh
   mkdir -p ~/pranata-inventory
   cp infra/inventory.example.yml ~/pranata-inventory/inventory.yml
   chmod 0600 ~/pranata-inventory/inventory.yml
   ```

   The inventory holds your domain, IP address and secrets. Never put it, or a
   copy of it, in the repository.

4. Tell the recipes where it is, in every shell where you run them:

   ```sh
   export PRANATA_INVENTORY=~/pranata-inventory/inventory.yml
   ```

### Fill in the inventory

Edit the one host entry. Rename `village-example` to something that identifies
the village (this is the `<host>` you pass to `just`).

| Variable                    | Meaning                                                                                  |
| --------------------------- | ---------------------------------------------------------------------------------------- |
| `ansible_host`              | The VPS IP address.                                                                      |
| `ansible_user`              | The sudo user you created. It must not be `root`.                                        |
| `ansible_port`              | Only if SSH is not on 22. The firewall keeps this port open.                              |
| `pranata_domain`            | Your domain, for example `<your-domain>`. Required.                                      |
| `pranata_version`           | The release to install, for example `0.1.0`. It is the tag of the published images. Required. |
| `pranata_tls`               | `acme` (Let's Encrypt through Caddy). `internal` is for tests only.                       |
| `pranata_village_code`      | The village's official region code. Used once, on the first run.                          |
| `pranata_admin_username`    | The first Administrator's sign-in name: 3 to 64 characters of `a-z`, `0-9`, `.`, `_`, `-`, starting with a letter or digit. |
| `pranata_admin_name`        | The first Administrator's display name.                                                  |
| `pranata_db_owner_password` | Password of the database owner role. Secret.                                             |
| `pranata_db_app_password`   | Password of the database role the API uses. Secret.                                      |
| `pranata_purge_token`       | Shared secret between the API and the web server for cache purges. Secret.               |
| `pranata_restic_password`   | Encrypts the backups. Secret. If you lose it, the backups are unreadable.                |
| `pranata_restic_repository` | Where backups go, for example `s3:https://<s3-endpoint>/<bucket>`.                        |
| `pranata_restic_env`        | `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` of the bucket key. Secret.               |
| `pranata_monitor_ping_urls` | Optional pings, see [Monitoring](#monitoring).                                           |

Provisioning stops with a message if a required variable is empty.

Other variables exist (for example `pranata_image_registry`, `pranata_health_timeout`
and the restic version). Their defaults are in
`infra/roles/pranata/defaults/main.yml`; most Installations change none of them.

### Generate and encrypt the secrets

Generate each secret separately. Hex has only digits and `a-f`, so it never
contains a quote, a backslash or a space:

```sh
openssl rand -hex 32
```

Use one value each for `pranata_db_owner_password`, `pranata_db_app_password`,
`pranata_purge_token` and `pranata_restic_password`. Store the restic password
somewhere outside the VPS and outside the inventory's only copy too, such as a
password manager.

Several of these values end up in environment files that systemd reads, where
quotes and backslashes would change them. The role therefore refuses a
`pranata_restic_repository`, `pranata_restic_password`, any `pranata_restic_env`
value or any `pranata_monitor_ping_urls` value that

- is empty (restic values only),
- contains a single quote, a double quote or a backslash anywhere,
- contains a line break, or
- starts or ends with whitespace.

If your bucket's secret key contains a quote or backslash, create a new key.
The purge token should use only letters, digits, `-` and `_`.

Encrypt each secret in the inventory with `ansible-vault`, in the version that
the project installs:

```sh
target/infra/bin/ansible-vault encrypt_string --stdin-name pranata_restic_password
```

Type or paste the value, press Ctrl-D twice, and paste the `!vault |` block it
prints in place of the placeholder, indented under the host:

```yaml
          pranata_restic_password: !vault |
            $ANSIBLE_VAULT;1.1;AES256
            ...
```

For nested values, encrypt `AWS_SECRET_ACCESS_KEY` and the others the same way
under `pranata_restic_env`.

Ansible needs the vault password every time you run a recipe. Either keep it in
a file outside the repository and export its path, or have Ansible ask:

```sh
export ANSIBLE_VAULT_PASSWORD_FILE=~/pranata-inventory/vault-password
# or
export ANSIBLE_ASK_VAULT_PASS=true
```

## Provision

Check that the release exists as images (`ghcr.io/dodoarmando/pranata-api` and
`pranata-web`, tagged with `pranata_version`), then run:

```sh
just provision <host>
```

`<host>` is the name of the host entry in your inventory, or `all`. The run
takes several minutes. It:

- upgrades the system and installs Podman, nftables, unattended security
  upgrades and a pinned `restic`;
- turns off SSH root login and password login, after checking that you connect
  as a non-root user with an authorized key;
- opens only SSH, 80 and 443 inbound;
- creates the unprivileged `pranata` user, which runs everything in rootless
  Podman under systemd (Postgres, the API, the web server and Caddy);
- starts the services and waits until they answer;
- on the first run only, initialises the Village with `pranata_village_code` and
  creates the first Administrator;
- installs the encrypted backup, the monthly restore check and the disk check,
  creates the restic repository in the bucket if it is empty, and starts the
  timers.

Running it again is safe. An Installation that already runs a release keeps that
release: provisioning never moves it. If the inventory's `pranata_version`
differs from what runs, you get a warning; use [Deploy](#deploy-a-new-version)
to move it.

### The first Administrator's Access Link

Near the end of the first run, Ansible prints one line:

```text
Access Link for <admin-username> (shown once, keep it safe): https://<your-domain>/akses#<token>
```

Copy it immediately. It is not stored on the server and not shown again.

1. Open the link in a browser. It opens `/akses#...`.
2. Set a password for the Administrator.
3. Sign in at `https://<your-domain>/masuk`.

The link works once and expires after 24 hours. Treat it like a password: send
it only to the Administrator. If it is lost, expired or already used, see the
next section.

## Lost Access Link or failed first run

Two situations need this: the link was lost or expired before the Administrator
used it, or the first run stopped before it created the Administrator (the
error says so). After fixing the cause, create an Administrator again:

```sh
just create-admin <host> <username> "<Display Name>"
```

For example `just create-admin <host> admin "Village Administrator"`. It prints
a new Access Link in the same way. Username rules are the same as above. If the
username already exists, the command fails with the reason; pick another
username. There is no screen for issuing a link to an existing user yet.

## Deploy a new version

Releases are published as images by the release workflow (`docs/release.md`).
To move an Installation to one:

```sh
just deploy <host> <version>
```

for example `just deploy <host> 0.2.0`, or `just deploy all 0.2.0` for every
Installation in the inventory. With `all`, hosts are updated one at a time, and
a failure stops the run before it reaches the next host.

For each host the deploy pulls the new images first (a missing tag changes
nothing), restarts the API, which applies database migrations, restarts the web
server, and then waits until `/api/healthz` reports the new version and the web
server answers. The wait lasts `pranata_health_timeout` seconds, 120 by default.

Afterwards, set `pranata_version` in your inventory to the new version so that
provisioning does not warn about a mismatch.

### When the health check fails: automatic rollback

The deploy returns the host to the version it ran before and ends with:

```text
Deploying <version> failed its health check; rolled back to <previous>.
```

The site is on the old version again. Find out why the new one did not start
before you retry:

```sh
ssh <ansible_user>@<vps-ip>
sudo -u pranata -H env XDG_RUNTIME_DIR=/run/user/$(id -u pranata) \
  journalctl --user -u pranata-api --since "30 minutes ago"
```

A failed migration of the new version stays in the log of that run. The schema
is designed so that the previous version keeps working on it, so the rollback is
safe. Report the failure, and deploy again only with a release that fixes it.

### When the rollback fails: ROLLBACK FAILED

If the previous version cannot start either, the deploy ends with:

```text
ROLLBACK FAILED on <host>: still on <version>; site may be down. Previous version was <previous>.
```

Act at once, the site may be down:

1. Open `https://<your-domain>` and check, then log in to the VPS (command above).
2. Read the state and logs:

   ```sh
   sudo -u pranata -H env XDG_RUNTIME_DIR=/run/user/$(id -u pranata) \
     systemctl --user status pranata-api pranata-web pranata-caddy pranata-postgres
   sudo -u pranata -H env XDG_RUNTIME_DIR=/run/user/$(id -u pranata) \
     podman logs pranata-api
   ```

3. Fix the cause. Typical ones are a full disk (`df -h /`), an unreachable image
   registry, or Postgres not running.
4. Run `just deploy <host> <previous>` again with the previous version from the
   message. If that is healthy, the site is back.

If you cannot find the cause, see [Restore from a backup](#backups-and-restore).

### Manual rollback

You can go back to an earlier release on purpose, for example when a problem
shows up after a deploy that passed its health check. Deploy the previous
version:

```sh
just deploy <host> <previous-version>
```

Rolling back never undoes database migrations. The migrations follow an
expand/contract scheme so that the previous release still runs on the newer
schema, but this is only guaranteed for one release back (D-50). If you need the
data as it was, restore a backup instead.

## Backups and restore

### What runs

Three systemd timers run as the `pranata` user (all persistent, so a VPS that
was off catches up):

| Timer                          | When                                    | What                                                                     |
| ------------------------------ | --------------------------------------- | ------------------------------------------------------------------------ |
| `pranata-backup.timer`         | daily at 02:00, up to 30 minutes late   | dumps the database and backs up the dump and the media directory with restic |
| `pranata-restore-check.timer`  | monthly, day 1 at 04:00                 | restores the latest dump into a scratch database, counts the Village rows, drops it |
| `pranata-disk-check.timer`     | hourly                                  | checks `/` is below 80 % full                                            |

Snapshots are tagged `pranata` and encrypted with `pranata_restic_password`
before they leave the VPS. Retention is 7 daily, 4 weekly and 12 monthly
snapshots; older ones are pruned after each backup. The whole media directory is
backed up, the private originals and the public image variants, together with the
database.

For the commands below, open a shell as the `pranata` user (its login shell is
disabled, so use `sudo` like this):

```sh
sudo -u pranata -H env XDG_RUNTIME_DIR=/run/user/$(id -u pranata) bash
cd ~
set -a; . ~/.config/pranata/backup.env; set +a
```

The last line loads the repository address, password and bucket key that the
timers use. It works because the role forbids quotes and backslashes in them.
Leave the shell with `exit`, which also forgets those variables.

### Run a backup now and list snapshots

```sh
systemctl --user start pranata-backup.service
journalctl --user -u pranata-backup.service -n 20
restic snapshots --tag pranata
```

Each snapshot has an ID, a time and the paths it holds: the dump
`/home/pranata/.local/state/pranata/backup/pranata.dump` and
`/home/pranata/media` (`originals/` and `public/`). Run the restore check on demand with
`systemctl --user start pranata-restore-check.service`.

### Inspect a backup in a scratch database

Use this to look at old data, or to prove a backup is good, without touching the
live database. It is the manual version of the monthly restore check.

```sh
podman exec pranata-postgres psql --username postgres --dbname postgres \
  --command "CREATE DATABASE pranata_scratch"

restic dump --tag pranata latest \
  /home/pranata/.local/state/pranata/backup/pranata.dump |
  podman exec --interactive pranata-postgres pg_restore --username postgres \
    --dbname pranata_scratch --no-owner --no-privileges --exit-on-error

podman exec pranata-postgres psql --username postgres --dbname pranata_scratch \
  --command '\dt' --command 'SELECT count(*) FROM village'
```

To use an older snapshot, replace `latest` with the ID from `restic snapshots`.
Look around with `psql` in the same way, then remove the scratch database:

```sh
podman exec pranata-postgres psql --username postgres --dbname postgres \
  --command "DROP DATABASE pranata_scratch WITH (FORCE)"
```

### Disaster restore onto a fresh VPS

Use this when the VPS is lost, or corrupted beyond repair. You need your private
inventory (with the restic and database secrets), the bucket, and a new VPS.
Provisioning is automated; the restore itself is a manual runbook. The scratch
restore above is exercised by the project's infra test, but this full procedure
is not rehearsed by CI, so practice it once on a spare VPS before you need it.

1. Create the new VPS and the sudo user as in [Before you start](#before-you-start).
   Point the domain's DNS record at its IP address and update `ansible_host` in
   the inventory. Keep every secret in the inventory unchanged, in particular
   `pranata_restic_repository`, `pranata_restic_password` and
   `pranata_restic_env`, and the database passwords.
2. Run `just provision <host>`. It installs everything and starts an empty
   Installation. Because there is no Village yet, it also creates a new Village
   and an Administrator and prints an Access Link. Ignore that link: the restore
   replaces that data. The run finds the existing restic repository in the
   bucket and keeps it.
3. On the VPS, open the `pranata` shell as in [What runs](#what-runs), then check that the snapshots are visible:

   ```sh
   restic snapshots --tag pranata
   ```

   Note the ID of the snapshot to restore, or use `latest` below.
4. Stop the API and the web server, so nothing writes to the database:

   ```sh
   systemctl --user stop pranata-web.service pranata-api.service
   ```

5. Replace the database with the dump. This deletes the empty one that
   provisioning created. The dump holds no database roles, so it is restored as
   the `pranata_owner` role that provisioning already created:

   ```sh
   podman exec pranata-postgres psql --username postgres --dbname postgres \
     --command "DROP DATABASE pranata WITH (FORCE)" \
     --command "CREATE DATABASE pranata OWNER pranata_owner"

   restic dump --tag pranata latest \
     /home/pranata/.local/state/pranata/backup/pranata.dump |
     podman exec --interactive pranata-postgres pg_restore --username postgres \
       --dbname pranata --no-owner --role pranata_owner --exit-on-error
   ```

6. Restore the media directory (`originals/` and `public/`). They belong to a subordinate user id, so restore
   them from inside the user namespace, as the backup reads them:

   ```sh
   podman unshare restic restore --tag pranata latest \
     --include /home/pranata/media --target /home/pranata/restore-tmp
   podman unshare mkdir -p /home/pranata/media
   podman unshare cp -a /home/pranata/restore-tmp/home/pranata/media/. \
     /home/pranata/media/
   podman unshare rm -rf /home/pranata/restore-tmp
   ```

   If the snapshot has no media directory (nothing had been uploaded yet), the
   restore finds nothing to include; skip this whole step.
7. Start the services and check the site:

   ```sh
   systemctl --user start pranata-api.service pranata-web.service
   exit
   ```

   Then open `https://<your-domain>` and sign in with an account from the
   backup. The Administrator that provisioning created is gone, and so are its
   sessions and Access Link.
8. Start a backup (`systemctl --user start pranata-backup.service`) and confirm
   a new snapshot appears.

What is not automated, and what to expect:

- Everything written after the last backup, up to a day, is lost.
- The restore needs the same release in the images as the dump was made with, or
  a newer one. If the inventory's `pranata_version` is older than the dump, deploy
  the right version afterwards with [Deploy](#deploy-a-new-version).

## Monitoring

Each Installation reports three things by pinging a URL (D-52). The ping format
is the one Healthchecks.io uses, so any service that speaks it works: a GET on
`<url>` means success, a GET on `<url>/fail` means failure.

| Key       | Pinged by                       | Schedule                          | Fails when                                            |
| --------- | ------------------------------- | --------------------------------- | ----------------------------------------------------- |
| `backup`  | `pranata-backup.service`        | daily at 02:00, up to 30 min late | the dump, `restic backup` or `forget` fails           |
| `restore` | `pranata-restore-check.service` | monthly on day 1 at 04:00         | the latest backup does not restore, or has no Village |
| `disk`    | `pranata-disk-check.service`    | hourly                            | `/` is more than 80 % full                            |

The units are systemd user units of the `pranata` user and their timers are
persistent, so a VPS that was off at 02:00 catches up when it starts. A failed
ping never fails the backup. A key without a URL is skipped.

The backup needs `pranata_restic_password` and `pranata_restic_repository` in
the inventory; provisioning refuses to run without them. While it runs, the
database dump is a temporary file in `~pranata/.local/state/pranata/backup/`
and counts toward the disk usage that the `disk` ping watches. It is removed
when the backup ends.

### Point the pings at a service

1. Create three checks in Healthchecks.io (hosted) or in a self-hosted
   Healthchecks instance. Give them periods that match the table with some
   grace: 1 day (grace 2 hours) for `backup`, 1 month (grace 1 day) for
   `restore`, 1 hour (grace 30 minutes) for `disk`.
2. Copy each check's ping URL into the inventory, ideally encrypted with
   `ansible-vault` because anyone holding a URL can report for the Installation:

   ```yaml
   pranata_monitor_ping_urls:
     backup: https://hc.example/ping/<uuid-of-backup-check>
     restore: https://hc.example/ping/<uuid-of-restore-check>
     disk: https://hc.example/ping/<uuid-of-disk-check>
   ```

3. Run `just provision <host>`. The URLs land in
   `~pranata/.config/pranata/monitor.env` (mode 0600).

### Verify the pings

Run each unit once as the `pranata` user and look at the check in the dashboard:

```sh
sudo -u pranata XDG_RUNTIME_DIR=/run/user/$(id -u pranata) systemctl --user start pranata-backup.service
sudo -u pranata XDG_RUNTIME_DIR=/run/user/$(id -u pranata) systemctl --user start pranata-restore-check.service
sudo -u pranata XDG_RUNTIME_DIR=/run/user/$(id -u pranata) systemctl --user start pranata-disk-check.service
```

Each check turns green. `systemctl --user list-timers` (same prefix) shows the
next runs, and `journalctl --user -u pranata-backup.service` the output of the
last one.

### Watch that the site is up

The three pings above run on the VPS, so they cannot tell you that the VPS or
the site is down: Healthchecks.io only waits for pings and never probes a URL.
Add an external uptime check from a hosted prober (Uptime Kuma, self-hosted
elsewhere, or any hosted monitor) with two checks, every minute or two:

| URL                            | Expect | Tells you                                                      |
| ------------------------------ | ------ | -------------------------------------------------------------- |
| `https://<domain>/`            | 200    | Caddy, the web server and the API behind it serve the home page |
| `https://<domain>/api/readyz`  | 200    | The API is up and the database answers                         |

`/api/readyz` answers 503 with the problem code `database_unavailable` when the
database does not. `/api/healthz` is only a liveness check (the process runs and
reports its version), so do not use it for uptime.

## Troubleshooting

### Provisioning refuses to start

| Message contains                                       | Cause and fix                                                                                          |
| ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------ |
| `Set pranata_domain, pranata_version, ...`             | A required variable is empty. Fill it in the inventory.                                                |
| `Ansible connects as root`                             | The SSH guard. `ansible_user` is `root`. Create the sudo user and set `ansible_user` (see Before you start). |
| `has no non-empty ~/.ssh/authorized_keys`              | The SSH guard. The connecting user has no key on the VPS. Run `ssh-copy-id <ansible_user>@<vps-ip>`, then run again. |
| `The sshd configuration with the Pranata drop-in did not validate` | The role removed its SSH change and did not reload sshd. Check `/etc/ssh/sshd_config` and `/etc/ssh/sshd_config.d/` on the VPS for errors, then run again. |
| `A pranata_restic_* value is empty, contains a quote, ...` | The env-value rule. Replace the value as described under [Generate and encrypt the secrets](#generate-and-encrypt-the-secrets). `no_log` hides which value it is; check the repository, password and bucket key in turn. |
| `A pranata_monitor_ping_urls value contains a quote, ...` | The same rule for the ping URLs. |
| `pranata_admin_username must be 3 to 64 characters ...` | Fix the username. Nothing was changed on the first run yet. |
| `pranata_village_code`                                 | Set it for the first run.                                                                              |

The SSH guard exists because disabling passwords for a user without a key would
lock you out. Do not skip it with `--skip-tags ssh` unless you know another way
into the VPS.

### The certificate is not issued

Symptoms: the browser shows a certificate error or no connection on
`https://<your-domain>`, though provisioning finished. The usual cause is that
the domain did not point at the VPS when Caddy first asked Let's Encrypt, or
ports 80 and 443 are blocked by the provider's own firewall.

```sh
dig +short <your-domain>
sudo -u pranata -H env XDG_RUNTIME_DIR=/run/user/$(id -u pranata) \
  podman logs pranata-caddy
```

`dig` must print the VPS IP. Fix the DNS record, wait for it to spread, then
restart Caddy so it retries:

```sh
sudo -u pranata -H env XDG_RUNTIME_DIR=/run/user/$(id -u pranata) \
  systemctl --user restart pranata-caddy.service
```

Let's Encrypt limits failed attempts per hour, so do not restart repeatedly.
The provisioning health check talks to Caddy locally and ignores the
certificate, so it passes even when the certificate is missing.

### Where the logs are

On the VPS, as the `pranata` user (the prefix sets the environment that the
user's systemd needs; each command is then written without it below):

```sh
sudo -u pranata -H env XDG_RUNTIME_DIR=/run/user/$(id -u pranata) bash
```

| What               | Command                                          |
| ------------------ | ------------------------------------------------ |
| API                | `journalctl --user -u pranata-api`               |
| Web server         | `journalctl --user -u pranata-web`               |
| Caddy              | `journalctl --user -u pranata-caddy`             |
| Postgres           | `journalctl --user -u pranata-postgres`          |
| Backups            | `journalctl --user -u pranata-backup.service`    |
| Restore check      | `journalctl --user -u pranata-restore-check.service` |
| Container output   | `podman logs pranata-api` (or `pranata-web`, `pranata-caddy`, `pranata-postgres`) |
| Unit state         | `systemctl --user status pranata-api`            |

Is the API up? `curl --fail https://<your-domain>/api/healthz` answers with the
running version.

## Security notes

- **Secrets live only in your private inventory**, encrypted with `ansible-vault`.
  On the VPS they are in files readable by the `pranata` user only (mode 0600).
  Back up the inventory and the vault password separately from the VPS. Do not
  commit them. A ping URL is also a credential.
- **Backups are encrypted, the disk is not.** restic encrypts every snapshot with
  `pranata_restic_password` before upload, so the bucket's operator cannot read
  them. Disk encryption on the VPS is deliberately deferred to R2, before
  population data arrives (ADR 0005, D-53). R1 holds almost no personal data.
  Keep the restic password outside the VPS; without it a restore is impossible.
- **CI never holds SSH keys.** CI builds and publishes images only. Every deploy
  runs from an operator's computer with their own SSH key (D-60), so a
  compromised repository cannot reach a village's server.
- **Access Links replace email.** The VPS sends no email; the first
  Administrator's link is shown once in your terminal (ADR 0007). Do not paste
  the output of a provisioning run into a ticket or chat.
- **Minimal exposure.** Only SSH, 80 and 443 accept connections, SSH accepts keys
  only, root cannot log in, and the application runs as an unprivileged user in
  rootless containers. Security updates install automatically.

## Maintainer notes

Cutting a release is described in `docs/release.md`. Before R1 is released,
someone other than the author must follow this guide on a fresh VPS, and the
result is recorded in the release pull request (D-54). Fix anything that
tripped them in this file.

