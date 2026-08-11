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

### Cluster / Distributed Mode

Set the same strong `KEY` on every instance. Distributed mode synchronizes room/channel metadata, empty channels, presence and profile state, moderation events, media toggles, and WebRTC signaling. Cluster events are loop-safe and can travel through intermediate RustRooms nodes.

#### Recommended: outbound relay topology

Servers do **not** need a direct IP route or an open inbound port to each other. Run one publicly reachable RustRooms instance as the relay, then point every private instance at it:

```text
# Public relay and all private nodes
KEY=a-long-random-shared-secret

# Each private node (the relay does not need this)
CLUSTER_RELAY_URL=wss://relay.example.com/cluster-ws
CLUSTER_DHT=false
```

Both private nodes make outbound WebSocket connections to the relay. The relay forwards state and signaling transitively, so users experience one logical server. Multiple comma-separated relay URLs may be supplied for redundant paths.

Configuration:

* `KEY`: Shared secret which enables cluster endpoints and authenticates peers.
* `CLUSTER_RELAY_URL`: One or more comma-separated RustRooms relay WebSocket URLs. Connections retry forever with backoff.
* `CLUSTER_PEERS`: Additional comma-separated peer URLs; equivalent to `CLUSTER_RELAY_URL` and useful for static meshes.
* `CLUSTER_DHT`: Enables legacy public DHT discovery. Defaults to `true`; set `false` for relay-only/private deployments.
* `CLUSTER_SCHEME`: Scheme for DHT peers and peer values without a scheme. `ws` by default; use `wss` over untrusted networks.

`https://` peer URLs are automatically converted to `wss://`, and a URL with no path automatically uses `/cluster-ws`. The relay must run this version of RustRooms and use the same `KEY`. TLS (`wss://`) is strongly recommended because profile data and signaling cross cluster links.

### Issues & Bug Reports

If you find a bug or issue, please [open an issue on GitHub](https://github.com/dillfrescott/RustRooms/issues).
