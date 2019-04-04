mod protos;

use std::env;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use grpc::{RequestOptions, SingleResponse};
use protos::ratelimit::{
    RateLimit, RateLimitRequest, RateLimitResponse, RateLimitResponse_Code,
    RateLimitResponse_DescriptorStatus, RateLimit_Unit,
};
use protos::ratelimit_grpc::RateLimitService;

use ratelimit_meter::{KeyedRateLimiter, LeakyBucket};

use env_logger::Env;
use log::{debug, error, info};
use nonzero_ext::nonzero;

enum RateLimitPlan {
    Paid = 100,
    Free = 10,
}

#[derive(Clone, Debug)]
struct RateLimitServiceImpl {
    limiter_paid: Arc<Mutex<KeyedRateLimiter<String, LeakyBucket>>>,
    limiter_free: Arc<Mutex<KeyedRateLimiter<String, LeakyBucket>>>,
}

impl RateLimitServiceImpl {
    pub fn create_service<H: RateLimitService + 'static + Sync + Send + 'static>(
        handler: H,
    ) -> ::grpc::rt::ServerServiceDefinition {
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
                    ::grpc::rt::MethodHandlerUnary::new(move |o, p| handler.should_rate_limit(o, p))
                },
            )],
        )
    }
}

impl RateLimitService for RateLimitServiceImpl {
    fn should_rate_limit(
        &self,
        _ctx: RequestOptions,
        req: RateLimitRequest,
    ) -> SingleResponse<RateLimitResponse> {
        let mut api_key: String = String::new();
        let mut user_plan: String = String::new();

        debug!("Domain: {}", req.get_domain());
        debug!("DescriptorsCount: {}", req.get_descriptors().len());

        for descriptor in req.get_descriptors() {
            debug!("-- New descriptor -- ");
            for entry in descriptor.entries.iter() {
                debug!("Descriptor Entry: [{}, {}]", entry.key, entry.value);

                if entry.key == "x-api-key" {
                    api_key = entry.value.clone();
                }
                if entry.key == "x-user-plan" {
                    user_plan = entry.value.clone();
                }
            }
        }

        let mut ratelimit = RateLimit::new();
        let mut descriptor_status = RateLimitResponse_DescriptorStatus::new();

        let code = if user_plan == "paid" {
            ratelimit.set_requests_per_unit(RateLimitPlan::Paid as u32);
            ratelimit.set_unit(RateLimit_Unit::SECOND);
            descriptor_status.set_current_limit(ratelimit);
            let arc_limiter_paid = self.limiter_paid.clone();
            let mut handle_paid = arc_limiter_paid.lock().unwrap();
            match handle_paid.check(api_key) {
                Ok(()) => RateLimitResponse_Code::OK,
                Err(_) => RateLimitResponse_Code::OVER_LIMIT,
            }
        } else {
            ratelimit.set_requests_per_unit(RateLimitPlan::Free as u32);
            ratelimit.set_unit(RateLimit_Unit::SECOND);
            descriptor_status.set_current_limit(ratelimit);
            let arc_limiter_free = self.limiter_free.clone();
            let mut handle_free = arc_limiter_free.lock().unwrap();
            match handle_free.check(api_key) {
                Ok(()) => RateLimitResponse_Code::OK,
                Err(_) => RateLimitResponse_Code::OVER_LIMIT,
            }
        };
        descriptor_status.set_code(code);

        let mut res = RateLimitResponse::new();
        res.mut_statuses().push(descriptor_status);
        res.set_overall_code(code);

        SingleResponse::completed(res)
    }
}

fn main() {
    env_logger::from_env(Env::default().default_filter_or("info")).init();

    let port = match env::var("PORT") {
        Ok(val) => val.parse().unwrap(),
        Err(_) => 50_051,
    };

    let rate_limiter = RateLimitServiceImpl {
        limiter_paid: Arc::new(Mutex::new(KeyedRateLimiter::<String, LeakyBucket>::new(
            nonzero!(RateLimitPlan::Paid as u32),
            Duration::from_secs(1),
        ))),
        limiter_free: Arc::new(Mutex::new(KeyedRateLimiter::<String, LeakyBucket>::new(
            nonzero!(RateLimitPlan::Free as u32),
            Duration::from_secs(1),
        ))),
    };

    let service = RateLimitServiceImpl::create_service(rate_limiter);
    let mut server = grpc::ServerBuilder::new_plain();
    server.http.set_port(port);
    server.add_service(service);
    let _server = server.build();

    info!("listening on port {}", port);

    loop {
        thread::park();
    }
}
