mod protos;

use std::io::Read;
use std::sync::Arc;
use std::{io, thread};

use futures::sync::oneshot;
use futures::Future;
use grpcio::{Environment, RpcContext, ServerBuilder, UnarySink};

use protos::ratelimit::{RateLimitRequest, RateLimitResponse};
use protos::ratelimit_grpc::{RateLimitService};

#[derive(Clone)]
struct RateLimiter;

impl RateLimitService for RateLimiter {

    fn should_rate_limit(&mut self, ctx: RpcContext,
                         req: RateLimitRequest, sink: UnarySink<RateLimitResponse>) {

    }
}

fn main() {
    let env = Arc::new(Environment::new(1));
    let service = protos::ratelimit_grpc::create_rate_limit_service(RateLimiter);
    let mut server = ServerBuilder::new(env)
        .register_service(service)
        .bind("127.0.0.1", 50_051)
        .build()
        .unwrap();
    server.start();

    for &(ref host, port) in server.bind_addrs() {
        println!("listening on {}:{}", host, port);
    }

    let (tx, rx) = oneshot::channel();
    thread::spawn(move || {
        println!("Press ENTER to exit...");
        let _ = io::stdin().read(&mut [0]).unwrap();
        tx.send(())
    });
    let _ = rx.wait();
    let _ = server.shutdown().wait();
}
