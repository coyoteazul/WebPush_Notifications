use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
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
#[serde(rename_all="camelCase")]
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
    require_interaction: Option<bool>,
    ///Prevent the notification from making noices or vibrations
    silent             : Option<bool>,
    ///Groups notificactions and allows to replace them
    tag                : Option<String>,
    ///Unix time in milliseconds. It defaults to the current time
    timestamp          : Option<u64>,
    ///https://developer.mozilla.org/en-US/docs/Web/API/Vibration_API#vibration_patterns
    vibrate            : Option<Vec<u16>>,
    ///https://angular.dev/ecosystem/service-workers/push-notifications
    /// Si el title es default, no se crea un nuevo boton
    actions            : Option<Vec<Action>>,
}

#[derive(Deserialize, ToSchema, Debug, Serialize)]
struct Action {
    title    : String,
    operation: Operation,
    url      : String,
}

///Copia de Notification pero con las actions adaptadas
#[derive(Serialize, Debug)]
struct NotifPush {
    title              : String,
    #[serde(skip_serializing_if = "Option::is_none")]
    badge              : Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body               : Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data               : Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon               : Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image              : Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lang               : Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    renotify           : Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    require_interaction: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    silent             : Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tag                : Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp          : Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vibrate            : Option<Vec<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    actions            : Option<Vec<ActionPush>>,
}

#[derive(Serialize, Debug)]
struct ActionPush {
    action: String,
    title : String
}

impl From<Notification> for NotifPush {
    fn from(value: Notification) -> Self {
        let Notification{ title, badge, body, data, icon, image, lang, renotify, require_interaction, silent, tag, timestamp, vibrate, actions } = value;
        let mut on_action_click = json!({});
        let mut count = 0;
        let mut d:Option<Value> = None;
        
        let a = actions.map(|val| {
            let actions: Vec<ActionPush> = val.into_iter().map(|row|{
                count +=1;

                let mut ret = ActionPush { action: format!("A{count}"), title:row.title };
                
                if ret.title == "default" {
                    ret.action = "default".to_owned();
                }
                
                let a = json!({"operation": row.operation, "url": row.url});
                on_action_click[ret.action.clone()] = a;

                ret
            })
            .collect();
            
            if actions.len() >0 {
                d = match data {
                    Some(mut data) => {
                        data["onActionClick"] = on_action_click;
                        Some(data)
                    },
                    None => {
                        Some(json!({"onActionClick":on_action_click}))
                    },
                };
            }
            

            actions
            .into_iter()
            .filter(|row| row.action != "default")
            .collect()
        })
        //convertir a None si array vacio
        .and_then(|a: Vec<ActionPush>| if a.is_empty() {None} else {Some(a)});


        Self { title, badge, body, data:d, icon, image, lang, renotify, require_interaction, silent, tag, timestamp, vibrate, actions:a}
    }
}

#[derive(Deserialize, ToSchema, Debug, Serialize)]
#[serde(rename_all="camelCase")]
enum Operation {
    OpenWindow,
    FocusLastFocusedOrOpen,
    NavigateLastFocusedOrOpen,
    SendRequest,
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
    

    //Armar Payload
    if req.payload.notification.timestamp.is_none() {
        req.payload.notification.timestamp = Some(Utc::now().timestamp_millis().try_into().unwrap())
    }

    let notif_push: NotifPush = req.payload.notification.into();
    let payload = json!({"notification": notif_push});
    let payload = dbg!(serde_json::to_string(&payload).unwrap()).into_bytes();

    // Build VAPID signature (set your mailto subject)
    let private_key = &conf.private_key;
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