use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::time::Duration;
use webrtc_util::conn::Conn;
use async_trait::async_trait;
use std::any::Any;

pub struct TurnAdapter {
    packet_rx: Mutex<mpsc::Receiver<(Vec<u8>, SocketAddr)>>,
    streams: Arc<Mutex<HashMap<SocketAddr, Arc<Mutex<TcpStream>>>>>,
    local_addr: SocketAddr,
}

impl TurnAdapter {
    pub fn new(local_addr: SocketAddr) -> (Arc<Self>, mpsc::Sender<TcpStream>) {
        let (packet_tx, packet_rx) = mpsc::channel(1000);
        let (stream_tx, mut stream_rx) = mpsc::channel::<TcpStream>(100);
        
        let adapter = Arc::new(Self {
            packet_rx: Mutex::new(packet_rx),
            streams: Arc::new(Mutex::new(HashMap::new())),
            local_addr,
        });
        
        let streams = adapter.streams.clone();
        tokio::spawn(async move {
            while let Some(stream) = stream_rx.recv().await {
                let peer_addr = match stream.peer_addr() {
                    Ok(a) => a,
                    Err(_) => continue,
                };
                
                let stream = Arc::new(Mutex::new(stream));
                streams.lock().await.insert(peer_addr, stream.clone());
                
                let packet_tx = packet_tx.clone();
                let streams_map = streams.clone();
                
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 65535];
                    loop {
                        let mut len_bytes = [0u8; 2];
                        let mut guard = stream.lock().await;
                        
                        let read_len = tokio::time::timeout(Duration::from_secs(10), guard.read_exact(&mut len_bytes)).await;
                        if let Ok(Ok(_)) = read_len {
                            let len = u16::from_be_bytes(len_bytes) as usize;
                            if len > buf.len() {
                                break;
                            }

                            let read_payload = tokio::time::timeout(Duration::from_secs(10), guard.read_exact(&mut buf[..len])).await;
                            if let Ok(Ok(_)) = read_payload {
                                drop(guard);
                                if packet_tx.send((buf[..len].to_vec(), peer_addr)).await.is_err() {
                                    break;
                                }
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    streams_map.lock().await.remove(&peer_addr);
                });
            }
        });
        
        (adapter, stream_tx)
    }
}

#[async_trait]
impl Conn for TurnAdapter {
    async fn connect(&self, _addr: SocketAddr) -> webrtc_util::Result<()> {
        Err(webrtc_util::Error::Other("Connect not supported".into()))
    }
    async fn recv(&self, _buf: &mut [u8]) -> webrtc_util::Result<usize> {
        Err(webrtc_util::Error::Other("recv not supported".into()))
    }
    async fn recv_from(&self, buf: &mut [u8]) -> webrtc_util::Result<(usize, SocketAddr)> {
        let mut rx = self.packet_rx.lock().await;
        if let Some((data, addr)) = rx.recv().await {
            if data.len() > buf.len() {
                return Err(webrtc_util::Error::ErrBufferShort);
            }
            buf[..data.len()].copy_from_slice(&data);
            Ok((data.len(), addr))
        } else {
             Err(webrtc_util::Error::Other("Closed pipe".into()))
        }
    }
    async fn send(&self, _buf: &[u8]) -> webrtc_util::Result<usize> {
        Err(webrtc_util::Error::Other("send not supported".into()))
    }
    async fn send_to(&self, buf: &[u8], target: SocketAddr) -> webrtc_util::Result<usize> {
        let streams = self.streams.lock().await;
        if let Some(stream) = streams.get(&target) {
            let mut guard = stream.lock().await;
            let len = buf.len() as u16;
            if let Err(e) = guard.write_all(&len.to_be_bytes()).await {
                 return Err(webrtc_util::Error::Other(e.to_string().into()));
            }
            if let Err(e) = guard.write_all(buf).await {
                return Err(webrtc_util::Error::Other(e.to_string().into()));
            }
            Ok(buf.len())
        } else {
             Ok(buf.len())
        }
    }
    fn local_addr(&self) -> webrtc_util::Result<SocketAddr> {
        Ok(self.local_addr)
    }
    fn remote_addr(&self) -> Option<SocketAddr> {
        None
    }
    async fn close(&self) -> webrtc_util::Result<()> {
        Ok(())
    }
    
    fn as_any(&self) -> &(dyn Any + Send + Sync) {
        self
    }
}