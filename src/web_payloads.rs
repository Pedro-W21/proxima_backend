use serde::{Deserialize, Serialize};

use crate::{ai_interaction::endpoint_api::{EndpointRequestVariant, EndpointResponseVariant}, database::{devices::DeviceType, DatabaseReplyVariant, DatabaseRequestVariant}};


#[derive(Clone, Serialize, Deserialize)]
pub struct AIPayload {
    auth_key:String,
    request:EndpointRequestVariant
}
#[derive(Clone, Serialize, Deserialize)]
pub struct AIResponse {
    reply:EndpointResponseVariant
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
    session_token:String,
}


#[derive(Clone, Serialize, Deserialize)]
pub struct DBPayload {
    auth_key:String,
    request:DatabaseRequestVariant
}
#[derive(Clone, Serialize, Deserialize)]
pub struct DBResponse {
    reply:DatabaseReplyVariant
}