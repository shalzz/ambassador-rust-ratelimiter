#[macro_use]
extern crate nonzero_ext;

mod protos;

use std::io::Read;
use std::{io, thread};
use std::time::Duration;

use futures::sync::oneshot;
use futures::Future;

use protos::ratelimit::{RateLimitRequest, RateLimitResponse};
use protos::ratelimit_grpc::{RateLimitService, RateLimitServiceServer};
use grpc::{RequestOptions, SingleResponse};

#[derive(Clone)]
struct RateLimitServiceImpl {
    handle: ratelimit::Handle,
}

impl RateLimitService for RateLimitServiceImpl {
    fn should_rate_limit(&self, ctx: RequestOptions, req: RateLimitRequest)
        -> SingleResponse<RateLimitResponse> {
        let res = RateLimitResponse::new();
        SingleResponse::completed(res)
    }
}

fn main() {
    let port = 50_051;
    let mut limiter = ratelimit::Builder::new()
        .capacity(1) //number of tokens the bucket will hold
        .quantum(1) //add one token per interval
        .interval(Duration::new(1, 0)) //add quantum tokens every 1 second
        .build();

    let rate_limiter = RateLimitServiceImpl {
        handle: limiter.make_handle()
    };
    let service = RateLimitServiceServer::new_service_def(rate_limiter);
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
