use std::sync::Arc;

use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

use crate::{database::{DatabaseItemID, DatabaseReplyVariant, DatabaseRequest, DatabaseRequestVariant}, proxima_handler::ProximaHandler};

use super::auth_web_handlers::is_auth_right;


#[derive(Clone, Serialize, Deserialize)]
pub struct DBPayload {
    auth_key:String,
    request:DatabaseRequestVariant
}
#[derive(Clone, Serialize, Deserialize)]
pub struct DBResponse {
    reply:DatabaseReplyVariant
}

pub async fn db_post_handler(payload: web::Json<DBPayload>, data: web::Data<Arc<ProximaHandler>>) -> impl Responder {
    if is_auth_right(payload.auth_key.clone(), data.clone()) {
        let (request, recv) = DatabaseRequest::new(payload.request.clone());
        data.database.send_prio(request);
        let reply = recv.recv().unwrap();
        HttpResponse::Ok().json(reply.variant)
    }
    else {
        HttpResponse::Forbidden().json("Wrong authentication")
    }
}