### How to use:

1. Install rust!

2.  From the repo dir just run `cargo build --release`.

3.  A standalone executable for your platform will be generated in ./target/release/

4.  Enjoy!

### Notes:

It uses port 3000 TCP (for the web interface) but can be changed by specifying the PORT env variable to a different value.

### TURN Configuration:

RustRooms requires a TURN server for WebRTC connections to work properly, especially when users are behind restrictive NATs or firewalls. You can configure a third-party TURN server using the following environment variables:

*   `TURN_URL`: The TURN server URL (e.g., `turn:your-turn-server.com:3478`)
*   `TURN_USERNAME`: The TURN server username
*   `TURN_CREDENTIAL`: The TURN server password/credential

For self-hosted TURN servers, you can use [coturn](https://github.com/coturn/coturn) or use a hosted service like [metered.ca](https://www.metered.ca).

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
REDIS_URLS=rediss://default:password@redis-1.example.com:6379,rediss://default:password@redis-2.example.com:6379
```

Upstash's TLS `rediss://` endpoint is recommended. With multiple endpoints, every instance publishes and subscribes through all of them; distributed traffic continues while any shared endpoint remains available. The Redis servers do not need Redis-level replication. Configure the same endpoint list on every RustRooms instance.

Connections retry independently with backoff, nodes exchange authoritative snapshots after reconnecting, and shared heartbeats remove presence only when a node is unreachable through every active Redis path. Large profiles are chunked before publication to stay below typical hosted Redis request limits.

Configuration:

* `REDIS_URL`: One URL or a comma-, semicolon-, or newline-separated list of standard `redis://` or TLS `rediss://` URLs.
* `REDIS_URLS`: Optional additional list using the same format. This is convenient for multiple endpoints.
* `REDIS_PREFIX`: Optional Pub/Sub namespace. Defaults to `rustrooms`. Set the same value on every instance, and use a different value when unrelated RustRooms deployments share Redis.

Redis is a coordination transport only; audio and video remain peer-to-peer or use the configured TURN server.

### Issues & Bug Reports

If you find a bug or issue, please [open an issue on GitHub](https://github.com/dillfrescott/RustRooms/issues).
