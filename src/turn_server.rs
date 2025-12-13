use anyhow::Result;
use std::net::SocketAddr;
use std::sync::Arc;
use turn::auth::{AuthHandler, generate_auth_key};
use turn::server::config::{ConnConfig, ServerConfig};
use turn::server::Server;
use turn::Error;
use turn::relay::relay_static::RelayAddressGeneratorStatic;
use webrtc_util::vnet::net::Net;

struct SimpleAuthHandler {
    user: String,
    key: Vec<u8>,
}

impl AuthHandler for SimpleAuthHandler {
    fn auth_handle(
        &self,
        username: &str,
        _realm: &str,
        _src_addr: SocketAddr,
    ) -> Result<Vec<u8>, Error> {
        if username == self.user {
            Ok(self.key.clone())
        } else {
            Err(Error::Other("Invalid user".into()))
        }
    }
}

use webrtc_util::conn::Conn;

pub async fn start(conn: Arc<dyn Conn + Send + Sync>, user: String, pass: String, realm: String) -> Result<()> {
    let public_ip = "0.0.0.0";

    let key = generate_auth_key(&user, &realm, &pass);

    let auth_handler = Arc::new(SimpleAuthHandler { 
        user: user.clone(), 
        key 
    });

    let relay_addr_gen = RelayAddressGeneratorStatic {
        relay_address: public_ip.parse()?,
        address: "0.0.0.0".to_owned(),
        net: Arc::new(Net::new(None)),
    };

    let config = ServerConfig {
        auth_handler,
        realm,
        conn_configs: vec![ConnConfig {
            conn,
            relay_addr_generator: Box::new(relay_addr_gen),
        }],
        channel_bind_timeout: std::time::Duration::from_secs(600),
        alloc_close_notify: None,
    };

    let server = Server::new(config).await?;
    println!("TURN Server started");

    tokio::signal::ctrl_c().await?;
    server.close().await?;

    Ok(())
}