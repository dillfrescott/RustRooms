use axum::extract::ws::Message;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};
use tokio::sync::Mutex;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UserStatus {
    pub nickname: String,
    pub avatar: Option<String>,
    pub is_gif: bool,
    pub static_frame: Option<String>,
    pub is_muted: bool,
    pub is_deafened: bool,
    pub is_screen_sharing: bool,
    #[serde(default)]
    pub is_low_bandwidth_mode: bool,
    #[serde(default)]
    pub is_on_the_go_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RoomStatus {
    pub(crate) name: String,
    pub(crate) users: HashMap<String, UserStatus>,
    #[serde(default)]
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SignalMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub target: Option<String>,
    pub data: Option<serde_json::Value>,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
}

pub(crate) type UserTx = tokio::sync::mpsc::Sender<Result<Message, axum::Error>>;
pub(crate) type ChannelMap = HashMap<String, HashMap<String, (UserTx, UserStatus)>>;
pub(crate) type RoomMap = Arc<Mutex<HashMap<String, ChannelMap>>>;
pub(crate) type RoomCleanupMap = Arc<Mutex<HashMap<String, u64>>>;
pub(crate) type RemoteUsersMap =
    Arc<Mutex<HashMap<String, HashMap<String, HashMap<String, UserStatus>>>>>;
pub(crate) type RemoteUserSourcesMap =
    Arc<Mutex<HashMap<(String, String, String), HashSet<String>>>>;
pub(crate) type ChannelCreationTimesMap = Arc<Mutex<HashMap<String, HashMap<String, u64>>>>;
pub(crate) const ROOM_EMPTY_GRACE_SECS: u64 = 120;
pub(crate) const MAX_ROOM_ID_LEN: usize = 64;
pub(crate) const MAX_CHANNEL_ID_LEN: usize = 32;
pub(crate) const MAX_NICKNAME_LEN: usize = 32;
// Avatar data URLs are relayed to every member of a channel and to every cluster
// node on every join/update, so the cap must stay small to bound amplification.
// 2 MiB of base64 is roughly 1.5 MiB of raw image data.
pub(crate) const MAX_AVATAR_DATA_LEN: usize = 2 * 1024 * 1024;
pub(crate) const MAX_STATIC_FRAME_DATA_LEN: usize = 512 * 1024;
pub(crate) const CLIENT_WS_MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;
// Cluster join messages contain the profile in both status and signaling data.
pub(crate) const CLUSTER_WS_MAX_MESSAGE_SIZE: usize = 32 * 1024 * 1024;
pub(crate) const OUTBOUND_QUEUE_CAPACITY: usize = 32;
pub(crate) const CLUSTER_BROADCAST_CAPACITY: usize = 64;
// Cluster links send an app-level keepalive frame every CLUSTER_KEEPALIVE_SECS
// and treat CLUSTER_PEER_TIMEOUT_SECS of total silence as a dead peer, so
// hard-crashed nodes don't leave ghost users in remote_users indefinitely.
pub(crate) const CLUSTER_KEEPALIVE_SECS: u64 = 5;
pub(crate) const CLUSTER_PEER_TIMEOUT_SECS: u64 = 30;
pub(crate) const MESSAGE_RATE_WINDOW_SECS: u64 = 10;
pub(crate) const MAX_MESSAGES_PER_RATE_WINDOW: u32 = 240;
// Byte budget per rate window: bounds JSON parse work even for huge frames.
// Plenty for a join (avatar + static frame) plus profile updates and signaling.
pub(crate) const MAX_BYTES_PER_RATE_WINDOW: usize = 16 * 1024 * 1024;
// Cap for relayed message payloads (signaling, cam/screen toggles, identify).
// Sized to fit a maximum-size avatar plus static frame.
pub(crate) const MAX_RELAY_DATA_LEN: usize = 3 * 1024 * 1024;
pub(crate) const PROFILE_IMAGE_UPDATE_COOLDOWN_SECS: u64 = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ClusterMessage {
    #[serde(rename = "type")]
    pub(crate) msg_type: String,
    pub(crate) room_id: String,
    pub(crate) channel_id: String,
    pub(crate) user_id: String,
    pub(crate) msg_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<UserStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) signal_msg: Option<String>,
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) rooms: RoomMap,
    pub(crate) room_cleanup_generations: RoomCleanupMap,
    pub(crate) room_creation_password: Option<String>,
    pub(crate) cluster_tx: tokio::sync::broadcast::Sender<String>,
    pub(crate) remote_users: RemoteUsersMap,
    pub(crate) remote_user_sources: RemoteUserSourcesMap,
    pub(crate) channel_creation_times: ChannelCreationTimesMap,
    pub(crate) cluster_key: Option<String>,
    pub(crate) cluster_scheme: String,
    pub(crate) allowed_url: Option<String>,
    pub(crate) connected_peers: Arc<Mutex<HashSet<String>>>,
    pub recent_cluster_msg_ids: Arc<Mutex<HashSet<String>>>,
    pub cluster_msg_history: Arc<Mutex<VecDeque<String>>>,
    pub(crate) node_id: String,
}

pub(crate) fn normalize_channel_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > MAX_CHANNEL_ID_LEN
        || trimmed.chars().any(char::is_control)
        || trimmed.contains(['/', '\\'])
    {
        return None;
    }

    if trimmed.eq_ignore_ascii_case("general") {
        Some("General".to_string())
    } else {
        Some(trimmed.to_string())
    }
}

pub(crate) fn is_valid_room_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= MAX_ROOM_ID_LEN
        && !value.chars().any(char::is_control)
        && !value.contains(['/', '\\'])
}

pub(crate) fn normalize_user_id(value: Option<&str>) -> String {
    value
        .and_then(|id| uuid::Uuid::parse_str(id).ok())
        .map(|id| id.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}

pub(crate) fn unique_user_id(mut candidate: String, is_occupied: impl Fn(&str) -> bool) -> String {
    while is_occupied(&candidate) {
        candidate = uuid::Uuid::new_v4().to_string();
    }
    candidate
}

pub(crate) fn current_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn normalize_configured_host(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let with_scheme = if value.contains("://") {
        value.to_string()
    } else {
        format!("http://{value}")
    };
    url::Url::parse(&with_scheme)
        .ok()?
        .host_str()
        .map(str::to_lowercase)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_ids_are_trimmed_and_general_is_canonicalized() {
        assert_eq!(
            normalize_channel_id("  lounge  ").as_deref(),
            Some("lounge")
        );
        assert_eq!(normalize_channel_id("gEnErAl").as_deref(), Some("General"));
    }

    #[test]
    fn invalid_channel_ids_are_rejected() {
        assert!(normalize_channel_id("   ").is_none());
        assert!(normalize_channel_id("line\nbreak").is_none());
        assert!(normalize_channel_id("path/segment").is_none());
        assert!(normalize_channel_id(&"a".repeat(MAX_CHANNEL_ID_LEN + 1)).is_none());
    }

    #[test]
    fn invalid_user_ids_are_replaced_with_uuids() {
        let normalized = normalize_user_id(Some("not-a-uuid"));
        assert!(uuid::Uuid::parse_str(&normalized).is_ok());

        let original = uuid::Uuid::new_v4();
        assert_eq!(
            normalize_user_id(Some(&original.to_string())),
            original.to_string()
        );
    }

    #[test]
    fn occupied_user_ids_are_replaced_without_reusing_the_active_identity() {
        let occupied = uuid::Uuid::new_v4().to_string();
        let replacement = unique_user_id(occupied.clone(), |id| id == occupied);

        assert_ne!(replacement, occupied);
        assert!(uuid::Uuid::parse_str(&replacement).is_ok());
    }

    #[test]
    fn configured_hosts_are_normalized_without_losing_ipv6() {
        assert_eq!(
            normalize_configured_host("https://Example.COM:8443/path").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            normalize_configured_host("[::1]:3000").as_deref(),
            Some("[::1]")
        );
    }
}
