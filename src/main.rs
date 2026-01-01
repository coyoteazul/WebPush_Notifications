use std::fs;

use axum::{Json, http::StatusCode, response::IntoResponse};
use log::info;
use serde::{Deserialize, Serialize};
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
     .routes(utoipa_axum::routes!(notify, get_public_key))
     .split_for_parts();

    router = router.route("/openapi.json", axum::routing::get(Json(api)));


    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, router).await.unwrap();
}


#[derive(Deserialize, ToSchema, Debug)]
struct SubscriptionKeys {
    p256dh: String,
    auth: String,
}

#[derive(Deserialize, ToSchema, Debug)]
struct Subscription {
    endpoint: String,
    keys: SubscriptionKeys,
}

#[derive(Deserialize, ToSchema, Debug)]
struct NotificationRequest {
    subscription: Subscription,
    payload: String,
}


#[derive(utoipa::IntoResponses,Deserialize,Serialize, ToSchema)]
enum NotifyResponses {
    /// Success response
    #[response(status = 200)]
    Ok(String),

    #[response(status = 404)]
    NotFound,

    #[response(status = 400)]
    BadRequest(String),
    #[response(status = 500)]
    InternalServerError(String),
}

impl IntoResponse for NotifyResponses {
    fn into_response(self) -> axum::response::Response {
        match self {
            NotifyResponses::Ok(msg) => (StatusCode::OK, Json(msg)).into_response(),
            NotifyResponses::NotFound => (StatusCode::NOT_FOUND, Json("Not Found")).into_response(),
            NotifyResponses::BadRequest(msg) => (StatusCode::BAD_REQUEST, Json(msg)).into_response(),
            NotifyResponses::InternalServerError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, Json(msg)).into_response(),
        }
    }
}

#[utoipa::path(post, path = "/notify", responses(NotifyResponses))]
async fn notify(Json(req): Json<NotificationRequest>) -> NotifyResponses {
    // Build subscription info
    dbg!(&req);
    let sub = SubscriptionInfo::new(
        req.subscription.endpoint,
        req.subscription.keys.p256dh,
        req.subscription.keys.auth,
    );

    // Load VAPID private key from file (adjust path if needed)
    let vapid_pem_path = r"keys.json";
    let pem = match fs::read(vapid_pem_path) {
        Ok(b) => b,
        Err(e) => {
            log::error!("Failed to read keys at {}: {}", vapid_pem_path, e);
            return NotifyResponses::InternalServerError("keys.json not found".into());
        }
    };

    let keys = match serde_json::from_slice::<KeysJson>(&pem) {
        Ok(k) => k,
        Err(e) => {
            log::error!("Failed to parse keys.json: {}", e);
            return NotifyResponses::InternalServerError("VAPID keys parse error".into());
        }
    };

    // Build VAPID signature (set your mailto subject)
    let sig = match VapidSignatureBuilder::from_base64(&keys.private_key, &sub) {
        Ok(b) => match b.build() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to build VAPID signature: {}", e);
                return NotifyResponses::InternalServerError("VAPID signature error".into());
            }
        },
        Err(e) => {
            log::error!("Failed to parse VAPID PEM: {}", e);
            return NotifyResponses::InternalServerError("VAPID PEM parse error".into());
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
            return NotifyResponses::InternalServerError("WebPush client error".into());
        }
    };

    match client.send(builder.build().unwrap()).await {
        Ok(_) => {
            info!("Push sent");
            NotifyResponses::Ok("Push sent successfully".into())
        }
        Err(e) => {
            log::error!("Failed to send push: {}", e);
            NotifyResponses::InternalServerError("Failed to send push".into())
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KeysJson {
    public_key: String,
    private_key: String,
}

#[derive(utoipa::IntoResponses,Deserialize,Serialize, ToSchema)]
enum GetPuKeyResponses {
    /// Success response
    #[response(status = 200)]
    Ok(String),

    #[response(status = 500)]
    InternalServerError(String),
}

impl IntoResponse for GetPuKeyResponses {
    fn into_response(self) -> axum::response::Response {
        match self {
            GetPuKeyResponses::Ok(msg) => (StatusCode::OK, Json(msg)).into_response(),
            GetPuKeyResponses::InternalServerError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, Json(msg)).into_response(),
        }
    }
}

#[utoipa::path(get, path = "/get_public_key", responses(GetPuKeyResponses))]
async fn get_public_key() -> GetPuKeyResponses {
    let vapid_pem_path = r"keys.json";
    let pem = match fs::read_to_string(vapid_pem_path) {
        Ok(b) => b,
        Err(e) => {
            log::error!("Failed to read keys at {}: {}", vapid_pem_path, e);
            return GetPuKeyResponses::InternalServerError("keys.json not found".into());
        }
    };
    
    let keys = match serde_json::from_str::<KeysJson>(&pem) {
        Ok(k) => k,
        Err(e) => {
            log::error!("Failed to parse keys.json: {}", e);
            return GetPuKeyResponses::InternalServerError("VAPID keys parse error".into());
        }
    };

    return GetPuKeyResponses::Ok(keys.public_key);
}