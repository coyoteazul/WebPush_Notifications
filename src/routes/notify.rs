use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::info;
use utoipa::ToSchema;
use web_push::{ContentEncoding, HyperWebPushClient, SubscriptionInfo, VapidSignatureBuilder, WebPushClient, WebPushMessageBuilder};

use crate::conf::KeysJson;

#[derive(Deserialize, ToSchema, Debug)]
struct SubscriptionKeys {
    p256dh: String,
    auth  : String,
}

#[derive(Deserialize, ToSchema, Debug)]
struct Subscription {
    endpoint: String,
    keys    : SubscriptionKeys,
}

#[derive(Deserialize, ToSchema, Debug)]
pub struct NotificationRequest {
    subscription: Subscription,
    payload     : PayLoad,
}

#[derive(Deserialize, ToSchema, Debug, Serialize)]
struct PayLoad {
    notification: Notification,
}


///https://developer.mozilla.org/en-US/docs/Web/API/Notification#Instance_properties
#[derive(Deserialize, ToSchema, Debug, Serialize)]
#[allow(non_snake_case)]
struct Notification {
    ///The title of the notification
    title              : String,
    ///A string containing the URL of an image to represent the notification when there is not enough space to display the notification itself
    badge              : Option<String>,
    ///The body string of the notification 
    body               : Option<String>,
    ///Json data to be used by the application
    data               : Option<Value>,
    ///The URL of the image used as an icon of the notification
    icon               : Option<String>,
    ///The URL of an image to be displayed as part of the notification
    image              : Option<String>,
    ///https://developer.mozilla.org/en-US/docs/Glossary/BCP_47_language_tag
    lang               : Option<String>,
    ///Specifies whether the user should be notified after a new notification replaces an old one.
    renotify           : Option<bool>,
    ///Prevent the notification from autoclosing without user interaction
    requireInteraction : Option<bool>,
    ///Prevent the notification from making noices or vibrations
    silent             : Option<bool>,
    ///Groups notificactions and allows to replace them
    tag                : Option<String>,
    ///Unix time in milliseconds. It defaults to the current time
    timestamp          : Option<u64>,
    ///https://developer.mozilla.org/en-US/docs/Web/API/Vibration_API#vibration_patterns
    vibrate            : Option<Vec<u16>>
}


#[derive(utoipa::IntoResponses,Deserialize,Serialize, ToSchema)]
pub enum NotifyResponses {
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
pub async fn notify(
    State(conf): State<Arc<KeysJson>>,
    Json(mut req): Json<NotificationRequest>,
) -> NotifyResponses {
    // Build subscription info
    info!("req: {:?}",&req);
    let sub = SubscriptionInfo::new(
        req.subscription.endpoint,
        req.subscription.keys.p256dh,
        req.subscription.keys.auth,
    );

    let private_key = &conf.private_key;
    
    if req.payload.notification.timestamp.is_none() {
        req.payload.notification.timestamp = Some(Utc::now().timestamp_millis().try_into().unwrap())
    }

    // Build VAPID signature (set your mailto subject)
    let sig = match VapidSignatureBuilder::from_base64(&private_key, &sub) {
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
    let payload = serde_json::to_string(&req.payload).unwrap().into_bytes();
    builder.set_payload(ContentEncoding::Aes128Gcm, &payload);
    builder.set_vapid_signature(sig);

    // Create client and send
    let client = HyperWebPushClient::new();

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