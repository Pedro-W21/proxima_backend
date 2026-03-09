use std::{sync::{Arc, mpsc::RecvTimeoutError}, time::Duration};

use actix_web::{HttpResponse, Responder, rt::spawn, web::{self, Bytes}};
use serde::{Deserialize, Serialize};

use proxima_backend::{database::{DatabaseInfoRequest, DatabaseItemID, DatabaseReplyVariant, DatabaseRequest, DatabaseRequestVariant, TunnelRequest}, proxima_handler::ProximaHandler};
use tokio::{sync::mpsc::{Receiver, Sender, channel}, time::sleep};
use tokio_stream::wrappers::ReceiverStream;

use crate::web_handlers::ai_endpoint_web_handlers::SpecialError;

use super::auth_web_handlers::is_auth_right;


use proxima_backend::web_payloads::{DBPayload, DBResponse};

pub async fn db_post_handler(payload: web::Json<DBPayload>, data: web::Data<Arc<ProximaHandler>>) -> impl Responder {
    if is_auth_right(payload.auth_key.clone(), data.clone()) {
        match payload.request.clone() {
            DatabaseRequestVariant::Info(DatabaseInfoRequest::UnknownUpdates { access_key }) => {
                let (request, recv) = TunnelRequest::new(access_key.clone());
                data.database.send_prio_tunnel(request);
                match recv.recv_timeout(Duration::from_millis(3000)) {
                    Ok(pending_updates) => {
                        let (sender, receiver):(Sender<Result<Bytes, SpecialError>>, Receiver<Result<Bytes, SpecialError>>) = channel(1000);
                        spawn(async move {
                            println!("[streaming updates to client] now starting to wait on updates");
                            loop {
                                match pending_updates.recv_timeout(Duration::from_millis(30)) {
                                    Ok(reply) => {
                                        match sender.send(Ok(web::Bytes::from_owner(serde_json::to_string(&reply).unwrap()))).await {
                                            Ok(_) => {
                                                println!("[streaming updates to client] Sent update to a client");
                                                continue;
                                            },
                                            Err(error) => {
                                                println!("[streaming updates to client] update sending failed, assuming client disconnected specific error : \n{error}");
                                                break;
                                            }
                                        }
                                    },
                                    Err(error) => match error {
                                        RecvTimeoutError::Disconnected => break,
                                        _ => ()
                                    }
                                }
                                sleep(Duration::from_millis(5000)).await;
                            }
                        });
                        let json = ReceiverStream::new(receiver);
                        HttpResponse::Ok().content_type("application/json").streaming(json)
                    },
                    Err(_) => HttpResponse::Forbidden().json("Wrong authentication")
                }
            },
            _ => {
                let (request, recv) = DatabaseRequest::new(payload.request.clone(), Some(payload.auth_key.clone()));
                data.database.send_prio(request);
                let reply = recv.recv().unwrap();
                HttpResponse::Ok().json(DBResponse {reply:reply.variant})
            }
        }
        
    }
    else {
        HttpResponse::Forbidden().json("Wrong authentication")
    }
}