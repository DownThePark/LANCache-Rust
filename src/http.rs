use log::{info, error};
use std::net::{TcpListener, SocketAddr};
use pingora::prelude::*;
use pingora::proxy::HttpProxy;
use crate::config::Config;
use crate::proxy::LANCache;

pub fn create_proxy_service(engine: &Server, cfg: &Config) -> Option<pingora::services::listening::Service<HttpProxy<LANCache>>> {
    let bind_addr = SocketAddr::new(cfg.http_bind.into(), 80);

    match TcpListener::bind(bind_addr) {
        Ok(_) => {
            info!("SUCCESS: LANCache HTTP listening on {}", bind_addr);
        }
        Err(e) => {
            error!("FAILED: Could not bind to {}: {}", bind_addr, e);
            error!("Ensure you have permission (sudo) and the address/port is available.");
            return None;
        }
    }

    let mut proxy_service = pingora::proxy::http_proxy_service(
        &engine.configuration, 
        LANCache { config: cfg.clone() }
    );
    proxy_service.add_tcp(&bind_addr.to_string());
    
    Some(proxy_service)
}
