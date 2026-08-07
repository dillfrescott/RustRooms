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

    let mut client_ip = String::new();
    if let Some(real_ip) = headers.get("X-Real-IP") {
        client_ip = real_ip.to_str().unwrap_or("").to_string();
    } else if let Some(forwarded_for) = headers.get("X-Forwarded-For") {
        client_ip = forwarded_for
            .to_str()
            .unwrap_or("")
            .split(',')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
    }

    ws.max_frame_size(CLIENT_WS_MAX_MESSAGE_SIZE)
        .max_message_size(CLIENT_WS_MAX_MESSAGE_SIZE)
        .on_upgrade(move |socket| handle_socket(socket, room_id, channel_id, state, client_ip))
}

#[cfg(test)]
mod tests {
    use super::*;

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
