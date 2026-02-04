mod config;
mod dns;
mod proxy;

use log::{info, warn, error};
use pingora::prelude::*;
use hickory_resolver::config::{ResolverConfig, ResolverOpts, NameServerConfig, Protocol};
use hickory_resolver::TokioAsyncResolver;
use std::net::TcpListener;
use crate::config::load_or_create_config;
use crate::proxy::LANCache;
use crate::dns::LANCacheDns;

#[tokio::main]
async fn main() {
    let cfg = load_or_create_config();

    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,pingora_core=warn,pingora_proxy=warn")
    )
    .format_timestamp(None)
    .format_module_path(false)
    .init();

    info!("------------------------------------------");
    info!("Starting LANCache (Rust Engine)");
    info!("Cache Location: {}", cfg.cache_dir);
    if cfg.dns_enabled {
        info!("Server IP:     {}", cfg.server_ip);
        info!("Upstream DNS:  {}", cfg.upstream_dns);
    }
    info!("------------------------------------------");

    // Pre-flight check for Port 80
    match TcpListener::bind("0.0.0.0:80") {
        Ok(_) => info!("SUCCESS: LANCache HTTP listening on port 80"),
        Err(e) => {
            error!("FAILED: Could not bind to port 80: {}", e);
            error!("Ensure you have permission (sudo) and no other service is using port 80.");
            return;
        }
    }

    let mut engine = Server::new(None).expect("Failed to initialize engine");
    let mut proxy_service = pingora::proxy::http_proxy_service(&engine.configuration, LANCache { config: cfg.clone() });
    proxy_service.add_tcp("0.0.0.0:80");
    engine.add_service(proxy_service);

    if cfg.dns_enabled {
        let upstream_addr = format!("{}:53", cfg.upstream_dns)
            .parse::<std::net::SocketAddr>()
            .expect("Invalid upstream_dns in config.toml");

        let mut resolver_config = ResolverConfig::new();
        resolver_config.add_name_server(NameServerConfig::new(upstream_addr, Protocol::Udp));

        let resolver = TokioAsyncResolver::tokio(resolver_config, ResolverOpts::default());
        let dns_handler = LANCacheDns { server_ip: cfg.server_ip, resolver };
        let mut dns_server = hickory_server::ServerFuture::new(dns_handler);
        
        let socket = match tokio::net::UdpSocket::bind("0.0.0.0:53").await {
            Ok(s) => {
                info!("SUCCESS: LANCache DNS listening on port 53");
                s
            },
            Err(e) => {
                error!("FAILED: Could not bind port 53: {}", e);
                return;
            }
        };
        dns_server.register_socket(socket);

        tokio::select! {
            _ = dns_server.block_until_done() => warn!("DNS service stopped."),
            _ = tokio::task::spawn_blocking(move || engine.run_forever()) => warn!("HTTP service stopped."),
            _ = tokio::signal::ctrl_c() => info!("Shutdown signal received."),
        }
    } else {
        tokio::select! {
            _ = tokio::task::spawn_blocking(move || engine.run_forever()) => warn!("HTTP service stopped."),
            _ = tokio::signal::ctrl_c() => info!("Shutdown signal received."),
        }
    }

    info!("Exiting LANCache. Goodbye!");
}
