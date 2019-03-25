extern crate grpc;

mod protos;

use std::io::Read;
use std::{io, thread};

use futures::sync::oneshot;
use futures::Future;

use protos::ratelimit::{RateLimitRequest, RateLimitResponse};
use protos::ratelimit_grpc::{RateLimitService, RateLimitServiceServer};
use grpc::RequestOptions;
use grpc::SingleResponse;

#[derive(Clone)]
struct RateLimiter;

impl RateLimitService for RateLimiter {
    fn should_rate_limit(&self, ctx: RequestOptions, req: RateLimitRequest)
        -> SingleResponse<RateLimitResponse> {
        let res = RateLimitResponse::new();
        SingleResponse::completed(res)
    }
}

fn main() {
    let port = 50_051;
    let service = RateLimitServiceServer::new_service_def(RateLimiter);
    let mut server = grpc::ServerBuilder::new_plain();
    server.http.set_port(port);
    server.add_service(service);
    let _server = server.build();

    println!("listening on port {}", port);

    let (tx, rx) = oneshot::channel();
    thread::spawn(move || {
        println!("Press ENTER to exit...");
        let _ = io::stdin().read(&mut [0]).unwrap();
        tx.send(())
    });
    let _ = rx.wait();
}
