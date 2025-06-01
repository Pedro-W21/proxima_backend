use std::sync::Arc;

use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

use crate::{ai_interaction::endpoint_api::{EndpointRequest, EndpointRequestVariant, EndpointResponseVariant}, database::{DatabaseItemID, DatabaseReplyVariant, DatabaseRequest, DatabaseRequestVariant}, proxima_handler::ProximaHandler};

use super::auth_web_handlers::is_auth_right;


#[derive(Clone, Serialize, Deserialize)]
pub struct AIPayload {
    auth_key:String,
    request:EndpointRequestVariant
}
#[derive(Clone, Serialize, Deserialize)]
pub struct AIResponse {
    reply:EndpointResponseVariant
}

pub async fn ai_post_handler(payload: web::Json<AIPayload>, data: web::Data<Arc<ProximaHandler>>) -> impl Responder {
    if is_auth_right(payload.auth_key.clone(), data.clone()) {
        let (request, recv) = EndpointRequest::new(payload.request.clone());
        data.ai_endpoint.send_prio(request);
        let reply = recv.recv().unwrap();
        HttpResponse::Ok().json(reply.variant)
    }
    else {
        HttpResponse::Forbidden().json("Wrong authentication")
    }
}