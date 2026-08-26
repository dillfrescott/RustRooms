### How to use:

1. Install rust!

2.  From the repo dir just run `cargo build --release`.

3.  A standalone executable for your platform will be generated in ./target/release/

4.  Enjoy!

### Notes:

It uses port 3000 TCP (for the web interface) but can be changed by specifying the PORT env variable to a different value.

### TURN Configuration:

RustRooms embeds a TURN server (`turn-server`, the relay used by LiveKit), so calls keep working behind restrictive NATs and firewalls without a third-party TURN provider. Every node runs the relay automatically on:

*   UDP 3478 (standard TURN port)
*   UDP 443 (the "web port": UDP is separate from the TCP HTTPS listener, and corporate firewalls commonly allow UDP 443 for QUIC/HTTP3)
*   UDP 49152-65535 (per-call relayed media ports)

Browsers fetch short-lived credentials from the app itself (`/turn`), so there is nothing to configure on the client. The relevant server-side environment variables are:

*   `TURN_SECRET`: Shared secret for the long-term credentials (HMAC-SHA1, RFC 5389 REST scheme). **Set the same value on every node.** If unset, a random per-boot secret is generated: fine for a single node, but credentials stop working after a restart and across distributed instances.
*   `TURN_PUBLIC_IP`: The public IPv4 clients use to reach the relay. Required on Fly.io (the VM's own IP is private and unreachable); on a VPS with a public IP the relay auto-detects it.

Deployment notes:

*   **Fly.io**: expose the UDP ports above in `fly.toml` (already included). UDP/443 is only useful if the edge does not claim it for HTTP/3; if `fly deploy` rejects the 443 service, drop it and keep 3478. If you scale to multiple machines, note that Fly does not pin UDP traffic to one VM, so TURN relays work reliably only on a single-machine deployment (or a dedicated machine with its own public IP).
*   **Distributed mode**: all nodes must share the same `TURN_SECRET` so credentials minted by any node validate on every other node's relay. Media then relays through whichever node the client reaches — the TURN layer is room-agnostic.
*   **Firewalls**: if you run behind your own firewall, open UDP 3478, UDP 443, and the relay range. TCP TURN and TURNS (TLS) are intentionally not exposed: TCP/443 needs TLS-in-app demultiplexing with browser caveats, and plaintext TCP TURN is blocked by most corporate firewalls anyway.

### Security:

For a production deployment, it is **highly recommended** to set the following environment variable:

*   `ROOM_CREATION_PASSWORD`: Set this to a strong password to prevent unauthorized room creation. If this is not set, anyone can create rooms.
*   `URL`: When set, restricts access to only requests whose `Host` header matches this value. Useful for preventing access via raw IP or alternative domain names. The value is automatically normalized (scheme and path are stripped).

### Redis Distributed Mode

Set the same `REDIS_URL` on every instance to make them one logical RustRooms deployment. The instances communicate through Redis Pub/Sub, so they do not need direct network paths or inbound coordination ports. Room/channel metadata, empty channels, presence and profile state, moderation events, media toggles, and WebRTC signaling are synchronized.

```text
# One Redis endpoint
REDIS_URL=rediss://default:password@your-upstash-host:6379

# Or two or more independent endpoints for redundancy
REDIS_URL=rediss://default:password@redis-1.example.com:6379,rediss://default:password@redis-2.example.com:6379
```

Upstash's TLS `rediss://` endpoint is recommended. With multiple endpoints, every instance publishes and subscribes through all of them; distributed traffic continues while any shared endpoint remains available. The Redis servers do not need Redis-level replication. Configure the same endpoint list on every RustRooms instance.

Connections retry independently with backoff, nodes exchange authoritative snapshots after reconnecting, and shared heartbeats remove presence only when a node is unreachable through every active Redis path. Large profiles are chunked before publication to stay below typical hosted Redis request limits.

Configuration:

* `REDIS_URL`: One URL or a comma-, semicolon-, or newline-separated list of standard `redis://` or TLS `rediss://` URLs.
* `REDIS_PREFIX`: Optional Pub/Sub namespace. Defaults to `rustrooms`. Set the same value on every instance, and use a different value when unrelated RustRooms deployments share Redis.

Redis is a coordination transport only; audio and video remain peer-to-peer or relay through the embedded TURN server.

### Issues & Bug Reports

If you find a bug or issue, please [open an issue on GitHub](https://github.com/dillfrescott/RustRooms/issues).
