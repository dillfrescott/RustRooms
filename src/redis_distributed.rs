use crate::{
    distributed::{
        cleanup_redis_node, distributed_user_data, process_redis_message, reconcile_redis_node,
        schedule_empty_room_cleanup,
    },
    state::*,
};
use futures::StreamExt;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use uuid::Uuid;

const PROTOCOL_VERSION: u8 = 1;
const REDIS_CHUNK_SIZE: usize = 256 * 1024;
const MAX_REDIS_FRAME_SIZE: usize = DISTRIBUTED_MAX_MESSAGE_SIZE;
const MAX_REDIS_CHUNKS: usize = MAX_REDIS_FRAME_SIZE.div_ceil(REDIS_CHUNK_SIZE);
const RECONNECT_MAX_SECS: u64 = 30;
const CHUNK_TTL_SECS: u64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum EnvelopeKind {
    Event,
    Heartbeat,
    SnapshotRequest,
    SnapshotStart,
    SnapshotEnd,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RedisEnvelope {
    version: u8,
    source_node_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    snapshot_id: Option<String>,
    kind: EnvelopeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<DistributedMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RedisChunk {
    version: u8,
    sender: String,
    transmission_id: String,
    index: usize,
    total: usize,
    payload: String,
}

struct ChunkAssembly {
    created_at: Instant,
    total: usize,
    bytes: usize,
    parts: Vec<Option<String>>,
}

struct SnapshotAssembly {
    created_at: Instant,
    users: HashSet<RemoteUserKey>,
}

type LastSeen = HashMap<String, Instant>;
type Assemblies = HashMap<(String, String), ChunkAssembly>;
type Snapshots = HashMap<(String, String), SnapshotAssembly>;

#[derive(Clone)]
struct RedisRuntime {
    last_seen: Arc<Mutex<LastSeen>>,
    assemblies: Arc<Mutex<Assemblies>>,
    snapshots: Arc<Mutex<Snapshots>>,
}

pub(crate) fn parse_redis_urls<'a>(values: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut urls = HashSet::new();
    for value in values {
        urls.extend(
            value
                .split([',', ';', '\n'])
                .map(str::trim)
                .filter(|url| !url.is_empty())
                .map(str::to_string),
        );
    }
    let mut urls: Vec<_> = urls.into_iter().collect();
    urls.sort();
    urls
}

pub(crate) fn spawn_redis_distributed(state: AppState, redis_urls: Vec<String>, prefix: String) {
    let channel = format!("{}:events:v1", prefix.trim().trim_end_matches(':'));
    let runtime = RedisRuntime {
        last_seen: Arc::new(Mutex::new(HashMap::new())),
        assemblies: Arc::new(Mutex::new(HashMap::new())),
        snapshots: Arc::new(Mutex::new(HashMap::new())),
    };

    for (index, redis_url) in redis_urls.into_iter().enumerate() {
        let client = match redis::Client::open(redis_url.as_str()) {
            Ok(client) => client,
            Err(error) => {
                eprintln!(
                    "REDIS DISTRIBUTED: Invalid Redis endpoint {}: {error}",
                    index + 1
                );
                continue;
            }
        };
        let label = redis_endpoint_label(&redis_url, index);
        let state = state.clone();
        let channel = channel.clone();
        let runtime = runtime.clone();
        let mut distributed_rx = state.distributed_tx.subscribe();
        tokio::spawn(async move {
            let mut retry_secs = 1u64;
            loop {
                match run_session(
                    &client,
                    &channel,
                    &label,
                    &state,
                    &mut distributed_rx,
                    &runtime,
                    &mut retry_secs,
                )
                .await
                {
                    Ok(()) => {
                        eprintln!("REDIS DISTRIBUTED [{label}]: Connection closed; reconnecting")
                    }
                    Err(error) => eprintln!(
                        "REDIS DISTRIBUTED [{label}]: Connection unavailable: {error}; retrying in {retry_secs}s"
                    ),
                }
                tokio::time::sleep(Duration::from_secs(retry_secs)).await;
                retry_secs = (retry_secs * 2).min(RECONNECT_MAX_SECS);
            }
        });
    }
}

fn redis_endpoint_label(redis_url: &str, index: usize) -> String {
    url::Url::parse(redis_url)
        .ok()
        .and_then(|url| {
            let host = url.host_str()?;
            Some(match url.port() {
                Some(port) => format!("{host}:{port}"),
                None => host.to_string(),
            })
        })
        .unwrap_or_else(|| format!("endpoint-{}", index + 1))
}

async fn run_session(
    client: &redis::Client,
    channel: &str,
    label: &str,
    state: &AppState,
    distributed_rx: &mut tokio::sync::broadcast::Receiver<String>,
    runtime: &RedisRuntime,
    retry_secs: &mut u64,
) -> redis::RedisResult<()> {
    let mut pubsub = client.get_async_pubsub().await?;
    pubsub.subscribe(channel).await?;
    let mut publisher = client.get_multiplexed_async_connection().await?;

    println!("REDIS DISTRIBUTED [{label}]: Connected and subscribed to {channel}");
    *retry_secs = 1;
    publish_envelope(
        &mut publisher,
        channel,
        &RedisEnvelope {
            version: PROTOCOL_VERSION,
            source_node_id: state.node_id.clone(),
            target_node_id: None,
            snapshot_id: None,
            kind: EnvelopeKind::Heartbeat,
            message: None,
        },
    )
    .await?;
    publish_envelope(
        &mut publisher,
        channel,
        &RedisEnvelope {
            version: PROTOCOL_VERSION,
            source_node_id: state.node_id.clone(),
            target_node_id: None,
            snapshot_id: None,
            kind: EnvelopeKind::SnapshotRequest,
            message: None,
        },
    )
    .await?;
    // Re-announce this node as well. This heals other instances that expired
    // its users during a Redis or process outage.
    publish_local_snapshot(&mut publisher, channel, state, None).await?;

    let mut heartbeat = tokio::time::interval(Duration::from_secs(REDIS_HEARTBEAT_SECS));
    heartbeat.tick().await;
    let mut liveness = tokio::time::interval(Duration::from_secs(REDIS_HEARTBEAT_SECS));
    liveness.tick().await;
    let mut messages = pubsub.on_message();

    loop {
        tokio::select! {
            received = messages.next() => {
                let Some(received) = received else {
                    return Ok(());
                };
                let raw: String = match received.get_payload() {
                    Ok(raw) => raw,
                    Err(error) => {
                        eprintln!("REDIS DISTRIBUTED [{label}]: Ignoring invalid Pub/Sub payload: {error}");
                        continue;
                    }
                };
                if let Some(envelope) = accept_chunk(&raw, &runtime.assemblies).await {
                    handle_envelope(
                        envelope,
                        &mut publisher,
                        channel,
                        state,
                        &runtime.last_seen,
                        &runtime.snapshots,
                    ).await?;
                }
            }
            event = distributed_rx.recv() => {
                match event {
                    Ok(raw) => {
                        let Ok(message) = serde_json::from_str::<DistributedMessage>(&raw) else {
                            continue;
                        };
                        publish_envelope(
                            &mut publisher,
                            channel,
                            &RedisEnvelope {
                                version: PROTOCOL_VERSION,
                                source_node_id: state.node_id.clone(),
                                target_node_id: None,
                                snapshot_id: None,
                                kind: EnvelopeKind::Event,
                                message: Some(message),
                            },
                        ).await?;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        eprintln!("REDIS DISTRIBUTED [{label}]: Outbound queue lagged by {skipped} event(s); publishing a fresh snapshot request");
                        publish_envelope(
                            &mut publisher,
                            channel,
                            &RedisEnvelope {
                                version: PROTOCOL_VERSION,
                                source_node_id: state.node_id.clone(),
                                target_node_id: None,
                                snapshot_id: None,
                                kind: EnvelopeKind::SnapshotRequest,
                                message: None,
                            },
                        ).await?;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return Ok(()),
                }
            }
            _ = heartbeat.tick() => {
                publish_envelope(
                    &mut publisher,
                    channel,
                    &RedisEnvelope {
                        version: PROTOCOL_VERSION,
                        source_node_id: state.node_id.clone(),
                        target_node_id: None,
                        snapshot_id: None,
                        kind: EnvelopeKind::Heartbeat,
                        message: None,
                    },
                ).await?;
            }
            _ = liveness.tick() => {
                cleanup_stale_nodes(state, &runtime.last_seen, &runtime.snapshots).await;
                runtime.assemblies.lock().await.retain(|_, assembly| {
                    assembly.created_at.elapsed() < Duration::from_secs(CHUNK_TTL_SECS)
                });
                runtime.snapshots.lock().await.retain(|_, snapshot| {
                    snapshot.created_at.elapsed() < Duration::from_secs(CHUNK_TTL_SECS)
                });
            }
        }
    }
}

async fn handle_envelope(
    envelope: RedisEnvelope,
    publisher: &mut redis::aio::MultiplexedConnection,
    channel: &str,
    state: &AppState,
    last_seen: &Arc<Mutex<LastSeen>>,
    snapshots: &Arc<Mutex<Snapshots>>,
) -> redis::RedisResult<()> {
    if envelope.version != PROTOCOL_VERSION
        || envelope.source_node_id == state.node_id
        || Uuid::parse_str(&envelope.source_node_id).is_err()
    {
        return Ok(());
    }

    // Targeted snapshot traffic still proves that its source is alive.
    last_seen
        .lock()
        .await
        .insert(envelope.source_node_id.clone(), Instant::now());
    if envelope
        .target_node_id
        .as_ref()
        .is_some_and(|target| target != &state.node_id)
    {
        return Ok(());
    }

    match envelope.kind {
        EnvelopeKind::Heartbeat => {}
        EnvelopeKind::SnapshotRequest => {
            publish_local_snapshot(
                publisher,
                channel,
                state,
                Some(envelope.source_node_id.clone()),
            )
            .await?;
        }
        EnvelopeKind::SnapshotStart => {
            let Some(snapshot_id) = valid_snapshot_id(envelope.snapshot_id) else {
                return Ok(());
            };
            snapshots.lock().await.insert(
                (envelope.source_node_id, snapshot_id),
                SnapshotAssembly {
                    created_at: Instant::now(),
                    users: HashSet::new(),
                },
            );
        }
        EnvelopeKind::SnapshotEnd => {
            let Some(snapshot_id) = valid_snapshot_id(envelope.snapshot_id) else {
                return Ok(());
            };
            let snapshot = snapshots
                .lock()
                .await
                .remove(&(envelope.source_node_id.clone(), snapshot_id));
            let Some(snapshot) = snapshot else {
                return Ok(());
            };
            let affected_rooms =
                reconcile_redis_node(state, &envelope.source_node_id, &snapshot.users).await;
            for room_id in affected_rooms {
                schedule_empty_room_cleanup(state, &room_id).await;
            }
        }
        EnvelopeKind::Event => {
            let Some(message) = envelope.message else {
                return Ok(());
            };
            if let Some(snapshot_id) = envelope.snapshot_id {
                let Some(snapshot_id) = valid_snapshot_id(Some(snapshot_id)) else {
                    return Ok(());
                };
                let mut snapshots = snapshots.lock().await;
                let Some(snapshot) =
                    snapshots.get_mut(&(envelope.source_node_id.clone(), snapshot_id))
                else {
                    return Ok(());
                };
                if message.msg_type == "user-joined" {
                    snapshot.users.insert((
                        message.room_id.clone(),
                        message.channel_id.clone(),
                        message.user_id.clone(),
                    ));
                }
            }
            process_redis_message(message, &envelope.source_node_id, state).await;
        }
    }
    Ok(())
}

fn valid_snapshot_id(snapshot_id: Option<String>) -> Option<String> {
    snapshot_id.filter(|id| Uuid::parse_str(id).is_ok())
}

async fn publish_envelope(
    connection: &mut redis::aio::MultiplexedConnection,
    channel: &str,
    envelope: &RedisEnvelope,
) -> redis::RedisResult<()> {
    let payload = serde_json::to_string(envelope).map_err(|error| {
        redis::RedisError::from((
            redis::ErrorKind::TypeError,
            "failed to serialize distributed message",
            error.to_string(),
        ))
    })?;
    if payload.len() > MAX_REDIS_FRAME_SIZE {
        eprintln!(
            "REDIS DISTRIBUTED: Dropping oversized distributed frame ({} bytes)",
            payload.len()
        );
        return Ok(());
    }

    let transmission_id = Uuid::new_v4().to_string();
    let parts = split_string(&payload, REDIS_CHUNK_SIZE);
    let total = parts.len();
    for (index, part) in parts.into_iter().enumerate() {
        let wire = serde_json::to_string(&RedisChunk {
            version: PROTOCOL_VERSION,
            sender: envelope.source_node_id.clone(),
            transmission_id: transmission_id.clone(),
            index,
            total,
            payload: part,
        })
        .map_err(|error| {
            redis::RedisError::from((
                redis::ErrorKind::TypeError,
                "failed to serialize Redis chunk",
                error.to_string(),
            ))
        })?;
        let _: i64 = connection.publish(channel, wire).await?;
    }
    Ok(())
}

async fn accept_chunk(raw: &str, assemblies: &Arc<Mutex<Assemblies>>) -> Option<RedisEnvelope> {
    let chunk: RedisChunk = serde_json::from_str(raw).ok()?;
    if chunk.version != PROTOCOL_VERSION
        || Uuid::parse_str(&chunk.sender).is_err()
        || Uuid::parse_str(&chunk.transmission_id).is_err()
        || chunk.total == 0
        || chunk.total > MAX_REDIS_CHUNKS
        || chunk.index >= chunk.total
        || chunk.payload.len() > REDIS_CHUNK_SIZE
    {
        return None;
    }

    let key = (chunk.sender.clone(), chunk.transmission_id.clone());
    let mut pending = assemblies.lock().await;
    let assembly = pending.entry(key.clone()).or_insert_with(|| ChunkAssembly {
        created_at: Instant::now(),
        total: chunk.total,
        bytes: 0,
        parts: (0..chunk.total).map(|_| None).collect(),
    });
    if assembly.total != chunk.total {
        pending.remove(&key);
        return None;
    }
    if assembly.parts[chunk.index].is_none() {
        assembly.bytes += chunk.payload.len();
        if assembly.bytes > MAX_REDIS_FRAME_SIZE {
            pending.remove(&key);
            return None;
        }
        assembly.parts[chunk.index] = Some(chunk.payload);
    }
    if assembly.parts.iter().any(Option::is_none) {
        return None;
    }

    let completed = pending.remove(&key)?;
    drop(pending);
    let mut payload = String::with_capacity(completed.bytes);
    for part in completed.parts {
        payload.push_str(part.as_deref()?);
    }
    let envelope: RedisEnvelope = serde_json::from_str(&payload).ok()?;
    (envelope.source_node_id == chunk.sender).then_some(envelope)
}

fn split_string(value: &str, max_bytes: usize) -> Vec<String> {
    if value.is_empty() {
        return vec![String::new()];
    }
    let mut parts = Vec::new();
    let mut start = 0;
    while start < value.len() {
        let mut end = (start + max_bytes).min(value.len());
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        parts.push(value[start..end].to_string());
        start = end;
    }
    parts
}

async fn publish_local_snapshot(
    publisher: &mut redis::aio::MultiplexedConnection,
    channel: &str,
    state: &AppState,
    target_node_id: Option<String>,
) -> redis::RedisResult<()> {
    let snapshot_id = Uuid::new_v4().to_string();
    publish_envelope(
        publisher,
        channel,
        &RedisEnvelope {
            version: PROTOCOL_VERSION,
            source_node_id: state.node_id.clone(),
            target_node_id: target_node_id.clone(),
            snapshot_id: Some(snapshot_id.clone()),
            kind: EnvelopeKind::SnapshotStart,
            message: None,
        },
    )
    .await?;

    let mut snapshot = local_snapshot_stream(state.clone());
    while let Some(message) = snapshot.recv().await {
        publish_envelope(
            publisher,
            channel,
            &RedisEnvelope {
                version: PROTOCOL_VERSION,
                source_node_id: state.node_id.clone(),
                target_node_id: target_node_id.clone(),
                snapshot_id: Some(snapshot_id.clone()),
                kind: EnvelopeKind::Event,
                message: Some(message),
            },
        )
        .await?;
    }
    publish_envelope(
        publisher,
        channel,
        &RedisEnvelope {
            version: PROTOCOL_VERSION,
            source_node_id: state.node_id.clone(),
            target_node_id,
            snapshot_id: Some(snapshot_id),
            kind: EnvelopeKind::SnapshotEnd,
            message: None,
        },
    )
    .await?;
    Ok(())
}

fn local_snapshot_stream(state: AppState) -> tokio::sync::mpsc::Receiver<DistributedMessage> {
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async move {
        let times = state.channel_creation_times.lock().await.clone();
        let rooms = state.rooms.lock().await;
        for (room_id, room) in rooms.iter() {
            for (channel_id, channel) in room {
                let created_at = times
                    .get(room_id)
                    .and_then(|channels| channels.get(channel_id))
                    .copied()
                    .unwrap_or(0);
                let channel_message = DistributedMessage {
                    msg_type: "channel-upsert".into(),
                    room_id: room_id.clone(),
                    channel_id: channel_id.clone(),
                    user_id: state.node_id.clone(),
                    msg_id: Uuid::new_v4().to_string(),
                    status: None,
                    data: Some(serde_json::json!({ "createdAt": created_at })),
                    signal_msg: None,
                };
                if tx.send(channel_message).await.is_err() {
                    return;
                }

                for (user_id, (_, status)) in channel {
                    let user_message = DistributedMessage {
                        msg_type: "user-joined".into(),
                        room_id: room_id.clone(),
                        channel_id: channel_id.clone(),
                        user_id: user_id.clone(),
                        msg_id: Uuid::new_v4().to_string(),
                        status: Some(status.clone()),
                        data: Some(distributed_user_data(status, created_at)),
                        signal_msg: None,
                    };
                    if tx.send(user_message).await.is_err() {
                        return;
                    }
                }
            }
        }
    });
    rx
}

async fn cleanup_stale_nodes(
    state: &AppState,
    last_seen: &Arc<Mutex<LastSeen>>,
    snapshots: &Arc<Mutex<Snapshots>>,
) {
    let stale: Vec<String> = {
        let mut last_seen = last_seen.lock().await;
        let stale: Vec<_> = last_seen
            .iter()
            .filter(|(_, seen)| seen.elapsed() > Duration::from_secs(REDIS_NODE_TIMEOUT_SECS))
            .map(|(node_id, _)| node_id.clone())
            .collect();
        for node_id in &stale {
            last_seen.remove(node_id);
        }
        stale
    };

    for node_id in stale {
        snapshots
            .lock()
            .await
            .retain(|(source_node_id, _), _| source_node_id != &node_id);
        let affected_rooms = cleanup_redis_node(state, &node_id).await;
        for room_id in affected_rooms {
            schedule_empty_room_cleanup(state, &room_id).await;
        }
        eprintln!("REDIS DISTRIBUTED: Node {node_id} timed out; removed its stale presence");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redis_url_lists_are_split_trimmed_and_deduplicated() {
        let urls = parse_redis_urls([
            "rediss://one.example:6379, rediss://two.example:6379",
            "rediss://one.example:6379;\nredis://three.example:6379",
        ]);
        assert_eq!(
            urls,
            vec![
                "redis://three.example:6379",
                "rediss://one.example:6379",
                "rediss://two.example:6379",
            ]
        );
    }

    #[test]
    fn endpoint_labels_never_include_redis_credentials() {
        let label = redis_endpoint_label("rediss://default:very-secret@redis.example.com:6380", 0);
        assert_eq!(label, "redis.example.com:6380");
    }

    #[test]
    fn chunks_round_trip_unicode_without_splitting_code_points() {
        let input = format!("{}🙂{}", "a".repeat(10), "b".repeat(10));
        let chunks = split_string(&input, 12);
        assert!(chunks.iter().all(|chunk| chunk.len() <= 12));
        assert_eq!(chunks.concat(), input);
    }
}
