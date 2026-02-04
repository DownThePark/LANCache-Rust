use async_trait::async_trait;
use hickory_proto::op::{Header, ResponseCode};
use hickory_proto::rr::{rdata::A, RData, Record, RecordType};
use hickory_resolver::TokioAsyncResolver;
use hickory_server::authority::MessageResponseBuilder;
use hickory_server::server::{Request, RequestHandler, ResponseHandler, ResponseInfo};
use log::info;
use std::net::Ipv4Addr;

#[derive(Clone)]
pub struct LANCacheDns {
    pub server_ip: Ipv4Addr,
    pub resolver: TokioAsyncResolver,
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
                response_handle.send_response(response).await.unwrap_or(r_info)
            }
            Err(_) => {
                let mut header = Header::response_from_request(request.header());
                header.set_response_code(ResponseCode::NXDomain);
                let builder = MessageResponseBuilder::from_message_request(request);
                let response = builder.build(header, std::iter::empty(), std::iter::empty(), std::iter::empty(), std::iter::empty());
                response_handle.send_response(response).await.unwrap_or_else(|_| (*request.header()).into())
            }
        }
    }
}
