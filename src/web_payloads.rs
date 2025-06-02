use serde::{Deserialize, Serialize};

use crate::{ai_interaction::endpoint_api::{EndpointRequestVariant, EndpointResponseVariant}, database::{devices::DeviceType, DatabaseReplyVariant, DatabaseRequestVariant}};


#[derive(Clone, Serialize, Deserialize)]
pub struct AIPayload {
    auth_key:String,
    request:EndpointRequestVariant
}
#[derive(Clone, Serialize, Deserialize)]
pub struct AIResponse {
    pub reply:EndpointResponseVariant
}

impl AIPayload {
    pub fn new(auth_key:String, request:EndpointRequestVariant) -> Self {
        Self { auth_key, request }
    }
}



#[derive(Clone, Serialize,Deserialize)]
pub struct AuthPayload {
    device_name:String,
    device_type:DeviceType,
    device_os:String,
    device_model:String,
    password_hash:String,
    username:String
}
#[derive(Clone, Serialize,Deserialize)]
pub struct AuthResponse {
    pub session_token:String,
}



impl AuthPayload {
    pub fn new(password:String, username:String) -> Self {
        Self {
            device_name: whoami::devicename(),
            device_type: match std::env::consts::OS {
                "android" | "ios" => DeviceType::Smartphone,
                _ => DeviceType::Laptop
            },
            device_os: std::env::consts::OS.into(),
            device_model: String::from("Generic computing device (I don't know man)"),
            password_hash: password,
            username: username 
        }
    }
}


#[derive(Clone, Serialize, Deserialize)]
pub struct DBPayload {
    auth_key:String,
    request:DatabaseRequestVariant
}

impl DBPayload {
    pub fn new(auth_key:String, request:DatabaseRequestVariant) -> Self {
        Self { auth_key, request }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DBResponse {
    reply:DatabaseReplyVariant
}