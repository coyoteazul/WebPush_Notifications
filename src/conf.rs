use std::fs;

use openssl::{base64, bn::BigNumContext, ec::{EcGroup, EcKey, PointConversionForm}, nid::Nid};
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
    // Create P-256 group and generate EC keypair
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1)?;
    let ec_key = EcKey::generate(&group)?;

    // Private key as DER (ASN.1 EC PRIVATE KEY)
    let priv_der = ec_key.private_key_to_der()?;

    // Public key as uncompressed point bytes
    let mut ctx = BigNumContext::new()?;
    let pub_point = ec_key.public_key();
    let pub_bytes = pub_point.to_bytes(&group, PointConversionForm::UNCOMPRESSED, &mut ctx)?;

    // URL-safe base64 without padding (commonly used for VAPID)
    let priv_b64 = base64::encode_block(&priv_der);
    let pub_b64 = base64::encode_block(&pub_bytes);

    Ok(KeysJson {
        public_key: pub_b64,
        private_key: priv_b64,
    })
}