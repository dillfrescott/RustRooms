use crate::{rooms::handle_socket, state::*, web_assets::get_html_page};
use axum::{
    extract::{Path, Query, State, ws::WebSocketUpgrade},
    http::header,
    response::{Html, IntoResponse, Redirect},
};
use std::collections::HashMap;
use uuid::Uuid;

fn request_host(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(header::HOST)?
        .to_str()
        .ok()?
        .parse::<axum::http::uri::Authority>()
        .ok()
        .map(|authority| authority.host().to_lowercase())
}

pub(crate) fn host_is_allowed(headers: &axum::http::HeaderMap, allowed_host: &str) -> bool {
    request_host(headers).is_some_and(|host| host.eq_ignore_ascii_case(allowed_host))
}

fn origin_matches_request_host(headers: &axum::http::HeaderMap) -> bool {
    let origin_host = headers
        .get(header::ORIGIN)
        .and_then(|origin| origin.to_str().ok())
        .and_then(|value| url::Url::parse(value).ok())
        .and_then(|url| url.host_str().map(str::to_lowercase));
    origin_host == request_host(headers)
}

pub(crate) async fn new_room(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Redirect, (axum::http::StatusCode, &'static str)> {
    if let Some(ref allowed_url) = state.allowed_url
        && !host_is_allowed(&headers, allowed_url)
    {
        return Err((axum::http::StatusCode::FORBIDDEN, "Forbidden"));
    }
    if let Some(ref required_pass) = state.room_creation_password {
        match params.get("password") {
            Some(p) if p == required_pass => {}
            _ => return Err((axum::http::StatusCode::UNAUTHORIZED, "Unauthorized")),
        }
    }

    let room_id = if let Some(custom_name) = params.get("name") {
        if custom_name.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            // Validate custom room name: alphanumeric, hyphens, underscores only, max length
            let trimmed = custom_name.trim();
            if trimmed.chars().count() > MAX_ROOM_ID_LEN
                || trimmed.is_empty()
                || !trimmed
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            {
                return Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    "Invalid room name: use only letters, numbers, hyphens, and underscores (max 64 characters)",
                ));
            }
            // Static routes shadow /{room_id}, so these names would redirect
            // or 404 instead of creating a room.
            if trimmed == "new" || trimmed == "cluster-ws" {
                return Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    "That room name is reserved. Please choose another.",
                ));
            }
            trimmed.to_string()
        }
    } else {
        Uuid::new_v4().to_string()
    };

    Ok(Redirect::to(&format!("/{}", room_id)))
}

pub(crate) async fn redirect_room_trailing_slash(Path(room_id): Path<String>) -> Redirect {
    // Validate before echoing into the Location header: control characters
    // (e.g. %0d%0a) would make Redirect::to panic.
    if is_valid_room_id(&room_id) {
        Redirect::to(&format!("/{}", room_id))
    } else {
        Redirect::to("/")
    }
}

pub(crate) async fn redirect_channel_trailing_slash(
    Path((room_id, channel_id)): Path<(String, String)>,
) -> Redirect {
    if is_valid_room_id(&room_id)
        && let Some(channel_id) = normalize_channel_id(&channel_id)
    {
        Redirect::to(&format!("/{}/{}", room_id, channel_id))
    } else {
        Redirect::to("/")
    }
}

pub(crate) async fn redirect_new_trailing_slash() -> Redirect {
    Redirect::to("/new")
}

pub(crate) async fn redirect_ws_trailing_slash(
    Path((room_id, channel_id)): Path<(String, String)>,
) -> Redirect {
    if is_valid_room_id(&room_id)
        && let Some(channel_id) = normalize_channel_id(&channel_id)
    {
        Redirect::to(&format!("/ws/{}/{}", room_id, channel_id))
    } else {
        Redirect::to("/")
    }
}

pub(crate) async fn index(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    if let Some(ref allowed_url) = state.allowed_url
        && !host_is_allowed(&headers, allowed_url)
    {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let html = get_html_page();

    let csp = "default-src 'self'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; script-src-elem 'self' 'unsafe-inline'; worker-src 'self' blob:; style-src 'self' 'unsafe-inline'; font-src 'self'; img-src 'self' data: https: blob:; connect-src 'self' wss: ws:; media-src 'self' blob:; object-src 'none'; frame-ancestors 'none';".to_string();

    (
        [(
            header::CONTENT_SECURITY_POLICY,
            axum::http::HeaderValue::from_str(&csp).unwrap(),
        )],
        Html(html),
    )
        .into_response()
}

pub(crate) async fn ws_handler(
    Path((room_id, channel_id)): Path<(String, String)>,
    Query(_): Query<HashMap<String, String>>,
    ws: WebSocketUpgrade,
    headers: axum::http::HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if !is_valid_room_id(&room_id) {
        return (axum::http::StatusCode::BAD_REQUEST, "Invalid room ID").into_response();
    }
    let Some(channel_id) = normalize_channel_id(&channel_id) else {
        return (axum::http::StatusCode::BAD_REQUEST, "Invalid channel ID").into_response();
    };
    if let Some(ref allowed_url) = state.allowed_url
        && !host_is_allowed(&headers, allowed_url)
    {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    if headers.contains_key(header::ORIGIN) && !origin_matches_request_host(&headers) {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden Origin").into_response();
    }

    ws.max_frame_size(CLIENT_WS_MAX_MESSAGE_SIZE)
        .max_message_size(CLIENT_WS_MAX_MESSAGE_SIZE)
        .on_upgrade(move |socket| handle_socket(socket, room_id, channel_id, state))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;
    use std::collections::{HashSet, VecDeque};
    use std::sync::Arc;

    fn test_state(password: Option<&str>) -> AppState {
        AppState {
            rooms: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            room_cleanup_generations: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            room_creation_password: password.map(str::to_string),
            cluster_tx: tokio::sync::broadcast::channel(CLUSTER_BROADCAST_CAPACITY).0,
            remote_users: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            remote_user_sources: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            remote_user_paths: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            channel_creation_times: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            cluster_key: None,
            cluster_scheme: "ws".to_string(),
            allowed_url: None,
            connected_peers: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            recent_cluster_msg_ids: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            cluster_msg_history: Arc::new(tokio::sync::Mutex::new(VecDeque::new())),
            node_id: Uuid::new_v4().to_string(),
        }
    }

    #[tokio::test]
    async fn new_room_rejects_missing_password_when_configured() {
        let state = test_state(Some("hunter2"));
        let err = new_room(State(state), HeaderMap::new(), Query(HashMap::new()))
            .await
            .unwrap_err();
        assert_eq!(err.0, axum::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn new_room_rejects_wrong_password() {
        let state = test_state(Some("hunter2"));
        let params = HashMap::from([("password".to_string(), "wrong".to_string())]);
        let err = new_room(State(state), HeaderMap::new(), Query(params))
            .await
            .unwrap_err();
        assert_eq!(err.0, axum::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn new_room_redirects_with_correct_password() {
        let state = test_state(Some("hunter2"));
        let params = HashMap::from([("password".to_string(), "hunter2".to_string())]);
        let res = new_room(State(state), HeaderMap::new(), Query(params)).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn new_room_allows_anyone_without_configured_password() {
        let state = test_state(None);
        let res = new_room(State(state), HeaderMap::new(), Query(HashMap::new())).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn new_room_rejects_disallowed_hosts() {
        let state = AppState {
            allowed_url: Some("example.com".to_string()),
            ..test_state(None)
        };
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "evil.example.net".parse().unwrap());
        let err = new_room(State(state), headers, Query(HashMap::new()))
            .await
            .unwrap_err();
        assert_eq!(err.0, axum::http::StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn new_room_rejects_reserved_names() {
        let state = test_state(None);
        for name in ["new", "cluster-ws"] {
            let params = HashMap::from([("name".to_string(), name.to_string())]);
            let err = new_room(State(state.clone()), HeaderMap::new(), Query(params))
                .await
                .unwrap_err();
            assert_eq!(err.0, axum::http::StatusCode::BAD_REQUEST);
        }
    }

    #[test]
    fn websocket_origin_must_match_the_request_host_exactly() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(header::HOST, "example.com".parse().unwrap());
        headers.insert(header::ORIGIN, "https://example.com".parse().unwrap());
        assert!(origin_matches_request_host(&headers));

        headers.insert(header::ORIGIN, "https://sub.example.com".parse().unwrap());
        assert!(!origin_matches_request_host(&headers));
    }
}
