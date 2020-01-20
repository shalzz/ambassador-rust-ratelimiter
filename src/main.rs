use std::env;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ratelimit_meter::{KeyedRateLimiter, LeakyBucket};

use env_logger::Env;
use log::{debug, info, trace};
use nonzero_ext::nonzero;

use tonic::{transport::Server, Request, Response, Status};

use ratelimit::rate_limit::Unit;
use ratelimit::rate_limit_response::{Code, DescriptorStatus};
use ratelimit::rate_limit_service_server::{RateLimitService, RateLimitServiceServer};
use ratelimit::{RateLimit, RateLimitRequest, RateLimitResponse};

pub mod ratelimit {
    tonic::include_proto!("pb.lyft.ratelimit");
}

enum RateLimitPlan {
    Paid = 200, // allows 100 rps with burst upto 200 rps
    Free = 20,  // allows 10 rps with burst upto 20 rps
}

#[derive(Debug)]
struct RateLimitServiceImpl {
    limiter_paid: Arc<Mutex<KeyedRateLimiter<String, LeakyBucket>>>,
    limiter_free: Arc<Mutex<KeyedRateLimiter<String, LeakyBucket>>>,
}

#[tonic::async_trait]
impl RateLimitService for RateLimitServiceImpl {
    async fn should_rate_limit(
        &self,
        req: Request<RateLimitRequest>,
    ) -> Result<Response<RateLimitResponse>, Status> {
        let mut remote_ip: String = String::from("default");
        let mut api_key: String = String::from("default");
        let mut user_plan: String = String::from("free");

        trace!("Domain: {}", req.get_ref().domain);
        trace!("DescriptorsCount: {}", req.get_ref().descriptors.len());

        for descriptor in req.into_inner().descriptors {
            trace!("-- New descriptor -- ");
            for entry in descriptor.entries.iter() {
                trace!("Descriptor Entry: [{}, {}]", entry.key, entry.value);

                if entry.key == "remote_address" {
                    remote_ip = entry.value.clone();
                }
                if entry.key == "xapiheader" {
                    api_key = entry.value.clone();
                }
                if entry.key == "xuserheader" {
                    user_plan = entry.value.clone();
                }
            }
        }

        debug!(
            "Got user {} with {} plan from ip {}",
            api_key, user_plan, remote_ip
        );

        let requests_per_unit;
        let code = if user_plan == "paid" {
            requests_per_unit = RateLimitPlan::Paid as u32;
            let arc_limiter_paid = Arc::clone(&self.limiter_paid);
            let mut handle_paid = arc_limiter_paid.lock().unwrap();
            match handle_paid.check(remote_ip) {
                Ok(()) => Code::Ok,
                Err(_) => Code::OverLimit,
            }
        } else {
            requests_per_unit = RateLimitPlan::Free as u32;
            let arc_limiter_free = Arc::clone(&self.limiter_free);
            let mut handle_free = arc_limiter_free.lock().unwrap();
            match handle_free.check(remote_ip) {
                Ok(()) => Code::Ok,
                Err(_) => Code::OverLimit,
            }
        };

        if code == Code::OverLimit {
            debug!("Ratelimiting!")
        }

        let ratelimit = RateLimit {
            requests_per_unit,
            unit: Unit::Second as i32,
        };
        let descriptor_status = DescriptorStatus {
            code: code as i32,
            current_limit: Some(ratelimit),
            limit_remaining: 0,
        };
        let res = RateLimitResponse {
            overall_code: code as i32,
            statuses: vec![descriptor_status],
        };

        Ok(Response::new(res))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::from_env(Env::default().default_filter_or("info")).init();
    trace!("Starting ratelimit service");

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

    let addr = format!("[::1]:{}", port).parse().unwrap();
    Server::builder()
        .add_service(RateLimitServiceServer::new(rate_limiter))
        .serve(addr)
        .await?;

    info!("listening on port {}", port);

    Ok(())
}
