use std::sync::Arc;

use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

use proxima_backend::{database::{DatabaseItemID, DatabaseReplyVariant, DatabaseRequest, DatabaseRequestVariant}, proxima_handler::ProximaHandler};

use super::auth_web_handlers::is_auth_right;


use proxima_backend::web_payloads::{DBPayload, DBResponse};

pub async fn home_get_handler(data: web::Data<Arc<ProximaHandler>>) -> impl Responder {
    HttpResponse::Ok()
}