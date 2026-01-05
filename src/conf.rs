use std::fs;

use base64::Engine;
use ::base64::prelude;
use openssl::{bn::BigNumContext, ec::{EcGroup, EcKey, PointConversionForm}, nid::Nid};
use serde::{Deserialize, Serialize};
use tracing::level_filters::LevelFilter;
use utoipa::openapi::Contact;


pub fn load_conf_file() -> ConfFile {
    let conf_path = r"conf.json";
    println!("Searching conf.json");
    match fs::read(conf_path) {
        Ok(b) => {
            println!("conf.json found");
            match serde_json::from_slice::<ConfFile>(&b) {
                Ok(k) => k,
                Err(e) => {
                    panic!("conf.json couldn't be parsed: {}", e);
                }
            }
        },
        Err(_) => {
            println!("conf.json couldn't be found. Creating new file, with newly made VAPID keys");

            let keys = generate_vapid_keys().unwrap();

            let conf = ConfFile { 
                openapi:OpenApi { 
                    title: "Webpush Notificator".to_owned(), 
                    description: "This sends notifications through webpush".to_owned(), 
                    version: "0.0.0".to_owned(), 
                    contact: Contact::new(),
                },
                keys,
                server: Server { 
                    trace_level: TraceLevel::TRACE,
                    accept_from: "0.0.0.0".to_owned(),
                    port: 1000,
                    api_key: "ApiKey_ArchiSecreta".to_owned()
                } 
            };
            let parsed = serde_json::to_string(&conf).unwrap();
            match fs::write(conf_path, parsed) {
                Ok(_) => {
                    conf
                },
                Err(err) => {
                    panic!("conf.json couldn't be saved: {}", err);
                },
            }  
        }
    }
}


#[derive(Deserialize, Serialize)]
pub struct ConfFile {
    pub openapi: OpenApi,
    pub keys   : KeysJson,
    pub server : Server,
}

#[derive(Deserialize, Serialize)]
pub struct OpenApi {
    pub title      : String,
    pub description: String,
    pub version    : String,
    pub contact    : utoipa::openapi::Contact,
}

#[derive(Deserialize, Serialize)]
pub struct Server {
    pub trace_level: TraceLevel,
    pub accept_from: String,
    pub port       : u16,
    pub api_key    : String,
}

#[derive(Deserialize, Serialize)]
pub struct KeysJson {
    pub public_key : String,
    pub private_key: String,
}

#[derive(Deserialize, Clone, Copy, Serialize)]
pub enum TraceLevel {
    DEBUG,
    INFO,
    TRACE,
}

impl Into<LevelFilter> for TraceLevel {
    fn into(self) -> LevelFilter {
        match self {
            TraceLevel::DEBUG => LevelFilter::DEBUG,
            TraceLevel::INFO  => LevelFilter::INFO,
            TraceLevel::TRACE => LevelFilter::TRACE,
        }
    }
}

// New: generate VAPID keypair suitable for web-push (P-256)
// returns KeysJson with base64 (URL-safe, no padding) encoded public and private key bytes.
fn generate_vapid_keys() -> Result<KeysJson, Box<dyn std::error::Error>> {
    // Generate EC keypair on P-256
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1)?;
    let ec_key = EcKey::generate(&group)?;

    // ---- PUBLIC KEY (65 bytes: 0x04 || X || Y) ----
    let mut ctx = BigNumContext::new()?;
    let pub_point = ec_key.public_key();
    let pub_bytes = pub_point.to_bytes(
        &group,
        PointConversionForm::UNCOMPRESSED,
        &mut ctx,
    )?;

    assert_eq!(pub_bytes.len(), 65);

    // ---- PRIVATE KEY (32-byte scalar d) ----
    let priv_bn = ec_key.private_key();
    let priv_bytes = priv_bn.to_vec_padded(32)?;

    // ---- Base64URL (no padding) ----
    let public_key = prelude::BASE64_URL_SAFE_NO_PAD.encode(&pub_bytes);
    let private_key = prelude::BASE64_URL_SAFE_NO_PAD.encode(&priv_bytes);

    Ok(KeysJson {
        public_key,
        private_key,
    })
}