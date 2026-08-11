mod cluster;
mod rooms;
mod routes;
mod state;
mod web_assets;

use axum::{Router, routing::get};
use cluster::{
    cluster_ws_handler, parse_cluster_peer_urls, spawn_dht_discovery, spawn_static_peer_connections,
};
use rooms::channel_status;
use routes::*;
use state::*;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};
use tokio::sync::Mutex;
use uuid::Uuid;
use web_assets::*;
#[tokio::main]
async fn main() {
    let rooms: RoomMap = Arc::new(Mutex::new(HashMap::new()));
    let room_cleanup_generations: RoomCleanupMap = Arc::new(Mutex::new(HashMap::new()));
    let channel_creation_times: ChannelCreationTimesMap = Arc::new(Mutex::new(HashMap::new()));

    let room_creation_password = std::env::var("ROOM_CREATION_PASSWORD")
        .ok()
        .map(|p| p.trim().to_string())
        .filter(|s| !s.is_empty());
    let cluster_key = std::env::var("KEY")
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|s| !s.is_empty());
    let cluster_scheme = std::env::var("CLUSTER_SCHEME")
        .ok()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| s == "wss")
        .unwrap_or_else(|| "ws".to_string());
    let allowed_url = std::env::var("URL")
        .ok()
        .and_then(|url| normalize_configured_host(&url));
    let configured_peer_urls = ["CLUSTER_PEERS", "CLUSTER_RELAY_URL"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .flat_map(|raw| parse_cluster_peer_urls(&raw, &cluster_scheme))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let dht_enabled = std::env::var("CLUSTER_DHT")
        .ok()
        .map(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "off" | "no"
            )
        })
        .unwrap_or(true);
    let (cluster_tx, _) = tokio::sync::broadcast::channel::<String>(CLUSTER_BROADCAST_CAPACITY);
    let remote_users: RemoteUsersMap = Arc::new(Mutex::new(HashMap::new()));
    let remote_user_sources: RemoteUserSourcesMap = Arc::new(Mutex::new(HashMap::new()));
    let remote_user_paths: RemoteUserPathsMap = Arc::new(Mutex::new(HashMap::new()));

    if cluster_key.is_some() {
        println!(
            "CLUSTER: Enabled via KEY env var ({} persistent peer(s), DHT: {}, default scheme: {})",
            configured_peer_urls.len(),
            if dht_enabled { "enabled" } else { "disabled" },
            cluster_scheme
        );
        let has_unencrypted_links = configured_peer_urls
            .iter()
            .any(|url| url.starts_with("ws://"))
            || (dht_enabled && cluster_scheme == "ws");
        if has_unencrypted_links {
            eprintln!("WARNING: CLUSTER: Using unencrypted ws:// for some inter-instance traffic.");
            eprintln!(
                "WARNING: Use wss:// peer URLs (and CLUSTER_SCHEME=wss for DHT) over untrusted networks."
            );
        }
    }

    if let Some(ref url) = allowed_url {
        println!(
            "URL RESTRICTION: Enabled - only allowing access from {}",
            url
        );
    }

    let node_id = Uuid::new_v4().to_string();

    let state = AppState {
        rooms,
        room_cleanup_generations,
        room_creation_password,
        cluster_tx,
        remote_users,
        remote_user_sources,
        remote_user_paths,
        channel_creation_times,
        cluster_key,
        cluster_scheme,
        allowed_url,
        connected_peers: Arc::new(Mutex::new(HashSet::new())),
        recent_cluster_msg_ids: Arc::new(Mutex::new(HashSet::new())),
        cluster_msg_history: Arc::new(Mutex::new(VecDeque::new())),
        node_id,
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/new", get(new_room))
        .route("/new/", get(redirect_new_trailing_slash))
        .route("/{room_id}", get(index))
        .route("/{room_id}/", get(redirect_room_trailing_slash))
        .route("/{room_id}/{channel_id}", get(index))
        .route(
            "/{room_id}/{channel_id}/",
            get(redirect_channel_trailing_slash),
        )
        .route("/{room_id}/{channel_id}/status", get(channel_status))
        .route("/rnnoise.js", get(rnnoise_js))
        .route("/rnnoise_processor.js", get(rnnoise_processor_js))
        .route("/manifest.json", get(manifest_json))
        .route("/service-worker.js", get(service_worker_js))
        .route("/icon.svg", get(icon_svg))
        .route("/assets/tailwind.js", get(tailwind_js))
        .route("/assets/tailwind-config.js", get(tailwind_config_js))
        .route("/assets/app.css", get(app_css))
        .route("/assets/app.js", get(app_js))
        .route("/assets/particles.js", get(particles_js))
        .route("/assets/croppie.min.js", get(croppie_js))
        .route("/assets/croppie.min.css", get(croppie_css))
        .route("/assets/inter.css", get(inter_css))
        .route(
            "/fonts/inter-cyrillic-ext.woff2",
            get(inter_cyrillic_ext_woff2),
        )
        .route("/fonts/inter-cyrillic.woff2", get(inter_cyrillic_woff2))
        .route("/fonts/inter-greek-ext.woff2", get(inter_greek_ext_woff2))
        .route("/fonts/inter-greek.woff2", get(inter_greek_woff2))
        .route("/fonts/inter-vietnamese.woff2", get(inter_vietnamese_woff2))
        .route("/fonts/inter-latin-ext.woff2", get(inter_latin_ext_woff2))
        .route("/fonts/inter-latin.woff2", get(inter_latin_woff2))
        .route("/ws/{room_id}/{channel_id}", get(ws_handler))
        .route(
            "/ws/{room_id}/{channel_id}/",
            get(redirect_ws_trailing_slash),
        )
        .route("/cluster-ws", get(cluster_ws_handler))
        .with_state(state.clone());

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3000);

    let listener = match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("ERROR: Failed to bind to port {}: {}", port, e);
            eprintln!("Is the server already running? Try killing the process using this port.");
            std::process::exit(1);
        }
    };
    println!("SERVER RUNNING ON PORT {}", port);

    if state.cluster_key.is_some() {
        spawn_static_peer_connections(state.clone(), configured_peer_urls);
        if dht_enabled {
            spawn_dht_discovery(state.clone(), port);
        }
    }

    axum::serve(listener, app).await.unwrap();
}
