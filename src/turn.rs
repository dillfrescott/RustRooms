use crate::state::current_unix_secs;
use base64::Engine;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr, UdpSocket},
};

/// How long a minted TURN credential stays valid. Clients fetch fresh
/// credentials per page load from `/turn`, so this only needs to outlive a
/// single call session.
pub(crate) const TURN_CREDENTIAL_TTL_SECS: u64 = 12 * 60 * 60;

const TURN_REALM: &str = "rustrooms";
// UDP listener ports. 3478 is the standard TURN port; 443 reuses the web
// port (the UDP space is separate from the TCP HTTPS listener) because
// corporate firewalls that block everything else commonly allow UDP 443
// (QUIC/HTTP3). A port that is already taken is skipped automatically.
const TURN_LISTEN_PORTS: [u16; 2] = [3478, 443];

/// Mint a TURN long-term credential pair (RFC 5389 / TURN REST API scheme):
/// username = expiry unix seconds, credential = base64(HMAC-SHA1(secret,
/// username)). This is byte-for-byte what turn-server's own
/// `static_auth_secret` verification expects, so any node that shares the
/// same `TURN_SECRET` accepts credentials minted by any other node.
pub(crate) fn turn_credential(secret: &str, ttl_secs: u64) -> (String, String) {
    let expiry = current_unix_secs() + ttl_secs;
    let username = expiry.to_string();
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, secret.as_bytes());
    let digest = ring::hmac::sign(&key, username.as_bytes());
    let credential = base64::engine::general_purpose::STANDARD.encode(digest.as_ref());
    (username, credential)
}

/// Start the embedded TURN relay on this node. `secret` must be the same on
/// all distributed instances (set `TURN_SECRET`); a per-boot random secret
/// still works on a single node but invalidates credentials across restarts.
pub(crate) fn spawn_turn_server(secret: String) {
    let external_ip = turn_public_ip();

    let mut interfaces = Vec::new();
    for port in TURN_LISTEN_PORTS {
        // Probe before handing the port to turn-server: a busy port aborts
        // the whole relay (all interfaces shut down when one fails), and
        // e.g. UDP 443 may already be taken on some hosts.
        if UdpSocket::bind(("0.0.0.0", port)).is_err() {
            eprintln!(
                "TURN SERVER: UDP port {port} unavailable; skipping (relay still runs on the other ports)"
            );
            continue;
        }
        interfaces.push(turn_server::config::Interface::Udp {
            listen: SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), port),
            // The address advertised to clients as the relay endpoint. On a
            // VPS the detected outbound IP is public; on Fly.io the VM's IP
            // is private, so set TURN_PUBLIC_IP to the app's public IPv4.
            external: SocketAddr::new(external_ip, port),
            idle_timeout: 20,
            mtu: 1500,
        });
    }
    if interfaces.is_empty() {
        eprintln!("TURN SERVER: no TURN ports could be bound; relay disabled");
        return;
    }

    let config = turn_server::config::Config {
        server: turn_server::config::Server {
            realm: TURN_REALM.to_string(),
            interfaces,
            port_range: Default::default(),
            max_threads: turn_server::config::Server::default().max_threads,
        },
        api: None,
        prometheus: None,
        hooks: None,
        log: turn_server::config::Log {
            level: turn_server::config::LogLevel::Error,
            stdout: false,
            file_directory: None,
        },
        auth: turn_server::config::Auth {
            static_credentials: HashMap::new(),
            static_auth_secret: Some(secret),
            enable_hooks_auth: false,
        },
    };

    tokio::spawn(async move {
        if let Err(error) = turn_server::start_server(config).await {
            eprintln!("TURN SERVER: fatal error, relay stopped: {error}");
        }
    });
}

/// The IP advertised as the relay address: explicit `TURN_PUBLIC_IP` wins,
/// otherwise the local outbound interface IP (correct on hosts with a public
/// IP, e.g. a bare VPS).
fn turn_public_ip() -> IpAddr {
    if let Some(configured) = std::env::var("TURN_PUBLIC_IP")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        && let Ok(parsed) = configured.parse::<IpAddr>()
    {
        return parsed;
    }

    // Detect the outbound interface IP with a throwaway UDP socket: connect
    // to a public address (no packets are sent) and read back the local
    // address the kernel picked. This is the machine's public IP on a typical
    // VPS; on Fly.io it is private, so operators there set TURN_PUBLIC_IP.
    let detected = UdpSocket::bind("0.0.0.0:0")
        .ok()
        .and_then(|socket| socket.connect("8.8.8.8:80").ok().map(|()| socket))
        .and_then(|socket| socket.local_addr().ok())
        .map(|addr| addr.ip());

    match detected {
        Some(ip) if !ip.is_unspecified() && !ip.is_loopback() => ip,
        other => {
            eprintln!(
                "TURN SERVER: could not determine a public IP (detected {other:?}); set TURN_PUBLIC_IP so relayed addresses are reachable"
            );
            IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 2202 test case 1: key = 0x0b repeated 20 times, data = "Hi There".
    #[test]
    fn hmac_sha1_matches_the_rfc_2202_vector() {
        let key = ring::hmac::Key::new(
            ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY,
            &[0x0b; 20],
        );
        let digest = ring::hmac::sign(&key, b"Hi There");
        assert_eq!(
            digest.as_ref(),
            &[
                0xb6, 0x17, 0x31, 0x86, 0x55, 0x05, 0x72, 0x64, 0xe2, 0x8b, 0xc0, 0xb6, 0xfb, 0x37,
                0x8c, 0x8e, 0xf1, 0x46, 0xbe, 0x00,
            ]
        );
    }

    #[test]
    fn credentials_use_expiry_usernames_and_base64_sha1_passwords() {
        let (username, credential) = turn_credential("s3cret", 3600);
        assert_eq!(username.parse::<u64>().unwrap(), current_unix_secs() + 3600);
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&credential)
            .unwrap();
        assert_eq!(decoded.len(), 20);
    }

    #[test]
    fn credentials_are_deterministic_per_secret_and_username() {
        let (username_a, cred_a) = turn_credential("s3cret", 60);
        let (username_b, cred_b) = turn_credential("s3cret", 60);
        assert_eq!(username_a, username_b);
        assert_eq!(cred_a, cred_b);

        let (_, cred_c) = turn_credential("different", 60);
        assert_ne!(cred_a, cred_c);
    }

    #[test]
    fn explicit_public_ip_wins_over_detection() {
        // SAFETY: tests run single-threaded per binary; no other thread reads
        // TURN_PUBLIC_IP while it is being mutated here.
        unsafe {
            std::env::set_var("TURN_PUBLIC_IP", "203.0.113.7");
        }
        assert_eq!(turn_public_ip(), "203.0.113.7".parse::<IpAddr>().unwrap());
        unsafe {
            std::env::remove_var("TURN_PUBLIC_IP");
        }
    }
}
