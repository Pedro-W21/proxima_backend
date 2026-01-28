use std::{fmt::Display, io::Stderr, sync::{Arc, mpsc::RecvTimeoutError}};

use actix_web::{HttpResponse, Responder, cookie::time::{Error, error::Format}, rt::{spawn, time::sleep}, web::{self, Bytes}};
use serde::{Deserialize, Serialize};

use proxima_backend::{ai_interaction::endpoint_api::{EndpointRequest, EndpointRequestVariant, EndpointResponseVariant}, database::{DatabaseItemID, DatabaseReplyVariant, DatabaseRequest, DatabaseRequestVariant}, proxima_handler::ProximaHandler};
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio_stream::wrappers::ReceiverStream;

use std::thread;
use std::time::Duration;
use futures::{future::ok, stream::iter};

use super::auth_web_handlers::is_auth_right;

use proxima_backend::web_payloads::{AIPayload, AIResponse};

use serde::ser::StdError;

enum SpecialError {

}

#[derive(Debug)]
pub struct SpecialStruct {

}

impl Display for SpecialStruct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

impl Into<Box<(dyn StdError + 'static)>> for SpecialError {
    fn into(self) -> Box<(dyn StdError + 'static)> {
        Box::new(Error::Format(Format::InvalidComponent("a")))
    }
}

pub async fn ai_post_handler(payload: web::Json<AIPayload>, data: web::Data<Arc<ProximaHandler>>) -> impl Responder {
    if is_auth_right(payload.auth_key.clone(), data.clone()) {
        let (request, recv) = EndpointRequest::new(payload.request.clone());
        data.ai_endpoint.send_prio(request);
        if payload.request.is_stream() {
            let (sender, receiver):(Sender<Result<Bytes, SpecialError>>, Receiver<Result<Bytes, SpecialError>>) = channel(1000);
            spawn(async move {
                loop {
                    println!("[streaming response to client] waiting on tokens");
                    match recv.recv_timeout(Duration::from_millis(10)) {
                        Ok(reply) => {
                            sender.send(Ok(web::Bytes::from_owner(serde_json::to_string(&reply.variant).unwrap()))).await;
                        },
                        Err(error) => match error {
                            RecvTimeoutError::Disconnected => break,
                            _ => ()
                        }
                    }
                    sleep(Duration::from_millis(10)).await;
                }
            });
            let json = ReceiverStream::new(receiver);
            HttpResponse::Ok().content_type("application/json").streaming(json)
        }
        else {
            let reply = recv.recv().unwrap();
            HttpResponse::Ok().json(AIResponse {reply:reply.variant})
        }
        
    }
    else {
        HttpResponse::Forbidden().json("Wrong authentication")
    }
}