# Pranata infrastructure

Ansible provisions one Installation per VPS (Debian 13). The full installation
guide is written separately; this file holds the parts that already exist.

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
