# Caddy on the host network, the api and web on loopback

On an Installation, the Caddy container runs with host networking (`Network=host`) and binds ports 80 and 443 itself. The api and web containers stay on the `pranata` bridge network, which they share with Postgres, and publish only on the loopback address: the api on `127.0.0.1:8080`, the web server on `127.0.0.1:3000`. Caddy proxies to those two addresses and does not set `trusted_proxies`.

Failed sign-ins are limited per client address (D-34), and the api takes that address from the `X-Forwarded-For` header Caddy writes. Caddy first ran on the bridge network with published ports. Rootless Podman forwards a published port through a user-space forwarder, which opens its own connection to the container, so every request reached Caddy from the forwarder's address. One client's failed sign-ins would then have throttled every resident and staff member together. With host networking there is no forwarder in front of Caddy: it sees each client's own address and writes it into `X-Forwarded-For`, replacing whatever the client sent. The role already lowers `net.ipv4.ip_unprivileged_port_start` to 80, so rootless Caddy can bind both ports.

## Considered Options

- **Pasta networking for Caddy, whose port forwarding can keep source addresses**: avoids host networking, but a pasta container cannot also join the bridge network, so Caddy would reach the api and web through published loopback ports all the same. Whether the address survives also depends on the pasta version and options: in the infra test, pasta rewrote the source of connections that came from the host itself.
- **Rootful Podman for Caddy**: root's port forwarding keeps addresses, but it puts a root-run container on every village's VPS, which the rootless setup exists to avoid.
- **`trusted_proxies` for the forwarder's address**: does not help, because the forwarder never sends the client's address; it would only let anyone who reaches Caddy through it choose their own address.

## Consequences

A local user on the VPS can reach the api and the web server directly on `127.0.0.1:8080` and `127.0.0.1:3000`, past Caddy, and could send any `X-Forwarded-For`. An Installation's VPS runs nothing but Pranata (ADR 0001), so that is accepted. Ports 8080 and 3000 on the VPS's loopback, and 80 and 443 on all its addresses, must be free. The firewall still opens only SSH, 80 and 443; loopback traffic never comes from outside. `infra/check.sh` fails if the Caddyfile sets `trusted_proxies`. The install guide, the stranger install test and the multi-Installation deploy (R1-E) must cover this deployment shape.
