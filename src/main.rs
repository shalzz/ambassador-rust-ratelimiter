#[macro_use]
extern crate nonzero_ext;

mod protos;

use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{io, thread};

use futures::sync::oneshot;
use futures::Future;

use grpc::{RequestOptions, SingleResponse};
use protos::ratelimit::{RateLimitRequest, RateLimitResponse, RateLimitResponse_Code};
use protos::ratelimit_grpc::RateLimitService;

use ratelimit_meter::{KeyedRateLimiter, LeakyBucket};

#[derive(Clone, Debug)]
struct RateLimitServiceImpl {
    limiter: Box<KeyedRateLimiter<String, LeakyBucket>>,
}

impl RateLimitService for RateLimitServiceImpl {
    fn should_rate_limit(
        &self,
        _ctx: RequestOptions,
        req: RateLimitRequest,
    ) -> SingleResponse<RateLimitResponse> {
        let mut handle = self.limiter.clone();
        let mut api_key: String = String::new();
        let mut user_plan: String = String::from("none");

        for descriptor in req.get_descriptors() {
            for entry in descriptor.entries.iter() {
                if entry.key == "x-api-key" {
                    api_key = entry.value.clone();
                }
                if entry.key == "x-user-plan" {
                    user_plan = entry.value.clone();
                }
            }
        }

        let code = if user_plan == "paid" {
            match handle.check(api_key) {
                Ok(()) => RateLimitResponse_Code::OK,
                Err(e) => RateLimitResponse_Code::OVER_LIMIT,
            }
        } else {
            RateLimitResponse_Code::OVER_LIMIT
        };

        let mut res = RateLimitResponse::new();
        res.set_overall_code(code);
        SingleResponse::completed(res)
    }
}

impl RateLimitServiceImpl {
    pub fn create_service<H: RateLimitService + 'static + Send + 'static>(
        handler: H,
    ) -> ::grpc::rt::ServerServiceDefinition {
        //let handler_arc = ::std::sync::Arc::new(Mutex::new(handler));
        let handler_mutex = Mutex::new(handler);
        ::grpc::rt::ServerServiceDefinition::new(
            "/pb.lyft.ratelimit.RateLimitService",
            vec![::grpc::rt::ServerMethod::new(
                Arc::new(::grpc::rt::MethodDescriptor {
                    name: "/pb.lyft.ratelimit.RateLimitService/ShouldRateLimit".to_string(),
                    streaming: ::grpc::rt::GrpcStreaming::Unary,
                    req_marshaller: Box::new(::grpc::protobuf::MarshallerProtobuf),
                    resp_marshaller: Box::new(::grpc::protobuf::MarshallerProtobuf),
                }),
                {
                    ::grpc::rt::MethodHandlerUnary::new(move |o, p| {
                        handler_mutex.lock().unwrap().should_rate_limit(o, p)
                    })
                },
            )],
        )
    }
}

fn main() {
    let port = 50_051;
    let rate_limiter = RateLimitServiceImpl {
        limiter: Box::new(KeyedRateLimiter::<String, LeakyBucket>::new(
            nonzero!(1u32),
            Duration::from_secs(5),
        )),
    };
    let service = RateLimitServiceImpl::create_service(rate_limiter);
    //let service = RateLimitServiceServer::new_service_def(rate_limiter);
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
