use async_trait::async_trait;
use hickory_proto::op::{Header, ResponseCode};
use hickory_proto::rr::{rdata::A, RData, Record, RecordType};
use hickory_resolver::config::{ResolverConfig, ResolverOpts, NameServerConfig, Protocol};
use hickory_resolver::TokioAsyncResolver;
use hickory_server::authority::MessageResponseBuilder;
use hickory_server::server::{Request, RequestHandler, ResponseHandler, ResponseInfo};
use log::{info, error};
use std::net::{Ipv4Addr, SocketAddr};
use crate::config::Config;

#[derive(Clone)]
pub struct LANCacheDns {
    pub server_ip: Ipv4Addr,
    pub resolver: TokioAsyncResolver,
}

pub async fn create_dns_service(cfg: &Config) -> Option<hickory_server::ServerFuture<LANCacheDns>> {
    let upstream_addr = format!("{}:53", cfg.upstream_dns)
        .parse::<SocketAddr>()
        .expect("Invalid upstream_dns in config.toml");

    let bind_addr = SocketAddr::new(cfg.dns_bind.into(), 53);

    let mut resolver_config = ResolverConfig::new();
    resolver_config.add_name_server(NameServerConfig::new(upstream_addr, Protocol::Udp));

    let resolver = TokioAsyncResolver::tokio(resolver_config, ResolverOpts::default());
    let dns_handler = LANCacheDns { server_ip: cfg.server_ip, resolver };
    let mut dns_server = hickory_server::ServerFuture::new(dns_handler);
    
    match tokio::net::UdpSocket::bind(bind_addr).await {
        Ok(socket) => {
            info!("SUCCESS: LANCache DNS listening on {}", bind_addr);
            dns_server.register_socket(socket);
            Some(dns_server)
        },
        Err(e) => {
            error!("FAILED: Could not bind to {}: {}", bind_addr, e);
            None
        }
    }
}

#[async_trait]
impl RequestHandler for LANCacheDns {
    async fn handle_request<R: ResponseHandler>(&self, request: &Request, mut response_handle: R) -> ResponseInfo {
        let query = request.request_info().query;
        let name_str = query.name().to_string();
        
        if name_str == "lancache.steamcontent.com." && query.query_type() == RecordType::A {
            info!("DNS INTERCEPT: {} -> {}", name_str, self.server_ip);
            let builder = MessageResponseBuilder::from_message_request(request);
            let header = Header::response_from_request(request.header());
            let record = Record::from_rdata(query.name().into(), 300, RData::A(A(self.server_ip)));
            let response = builder.build(header, std::iter::once(&record), std::iter::empty(), std::iter::empty(), std::iter::empty());
            let r_info = ResponseInfo::from(*response.header());
            return response_handle.send_response(response).await.unwrap_or(r_info);
        }

        match self.resolver.lookup(query.name(), query.query_type()).await {
            Ok(lookup) => {
                let builder = MessageResponseBuilder::from_message_request(request);
                let header = Header::response_from_request(request.header());
                let records: Vec<Record> = lookup.records().to_vec();
                let response = builder.build(header, records.iter(), std::iter::empty(), std::iter::empty(), std::iter::empty());
                let r_info = ResponseInfo::from(*response.header());
                _ = response_handle.send_response(response).await;
                r_info
            }
            Err(_) => {
                let mut header = Header::response_from_request(request.header());
                header.set_response_code(ResponseCode::NXDomain);
                let builder = MessageResponseBuilder::from_message_request(request);
                let response = builder.build(header, std::iter::empty(), std::iter::empty(), std::iter::empty(), std::iter::empty());
                _ = response_handle.send_response(response).await;
                (*request.header()).into()
            }
        }
    }
}
