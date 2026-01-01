use std::fs;

use axum::{Json, http::StatusCode, response::IntoResponse};
use log::info;
use serde::Deserialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use web_push::{ContentEncoding, IsahcWebPushClient, SubscriptionInfo, VapidSignatureBuilder, WebPushClient, WebPushMessageBuilder};

#[tokio::main]
async fn main() {
    //env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));    

    tracing_subscriber::fmt()
    .with_max_level(tracing::Level::TRACE)
    .init();

    // Build app
     let (mut router, api): (axum::Router, utoipa::openapi::OpenApi) = OpenApiRouter::new()
     .routes(utoipa_axum::routes!(notify))
     .split_for_parts();

    router = router.route("/openapi.json", axum::routing::get(Json(api)));


    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, router).await.unwrap();
}


#[derive(Deserialize, ToSchema)]
struct SubscriptionKeys {
    p256dh: String,
    auth: String,
}

#[derive(Deserialize, ToSchema)]
struct Subscription {
    endpoint: String,
    keys: SubscriptionKeys,
}

#[derive(Deserialize, ToSchema)]
struct NotificationRequest {
    subscription: Subscription,
    payload: String,
}


#[derive(utoipa::IntoResponses)]
enum NotifyResponses {
    /// Success response
    #[response(status = 200)]
    Ok(NotificationRequest),

    #[response(status = 404)]
    NotFound,

    #[response(status = 400)]
    BadRequest(String),
}

#[utoipa::path(post, path = "/notify", responses(NotifyResponses))]
async fn notify(Json(req): Json<NotificationRequest>) -> impl IntoResponse {
    // Build subscription info
    let sub = SubscriptionInfo::new(
        req.subscription.endpoint,
        req.subscription.keys.p256dh,
        req.subscription.keys.auth,
    );

    // Load VAPID private key from file (adjust path if needed)
    let vapid_pem_path = r"vapid_private.pem";
    let pem = match fs::read(vapid_pem_path) {
        Ok(b) => b,
        Err(e) => {
            log::error!("Failed to read VAPID private key at {}: {}", vapid_pem_path, e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "VAPID private key not found");
        }
    };

    // Build VAPID signature (set your mailto subject)
    let sig = match VapidSignatureBuilder::from_pem(&*pem, &sub) {
        Ok(b) => match b.build() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to build VAPID signature: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "VAPID signature error");
            }
        },
        Err(e) => {
            log::error!("Failed to parse VAPID PEM: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "VAPID PEM parse error");
        }
    };

    // Create message builder and optional payload
    let mut builder = WebPushMessageBuilder::new(&sub);
    let payload = req.payload.into_bytes();
    builder.set_payload(ContentEncoding::Aes128Gcm, &payload);
    builder.set_vapid_signature(sig);

    // Create client and send
    let client = match IsahcWebPushClient::new() {
        Ok(c) => c,
        Err(e) => {
            log::error!("Failed to create WebPushClient: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "WebPush client error");
        }
    };

    match client.send(builder.build().unwrap()).await {
        Ok(_) => {
            info!("Push sent");
            (StatusCode::OK, "Push sent")
        }
        Err(e) => {
            log::error!("Failed to send push: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to send push")
        }
    }
}

