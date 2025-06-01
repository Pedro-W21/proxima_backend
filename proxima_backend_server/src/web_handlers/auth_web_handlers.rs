use std::sync::Arc;

use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

use proxima_backend::{database::{devices::{Device, DeviceType}, DatabaseInfoReply, DatabaseInfoRequest, DatabaseItem, DatabaseItemID, DatabaseReplyVariant, DatabaseRequest, DatabaseRequestVariant}, proxima_handler::ProximaHandler};

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

pub async fn auth_post_handler(payload: web::Json<AuthPayload>, data: web::Data<Arc<ProximaHandler>>) -> impl Responder {
    // process payload and use handler
    let (request, recv) = DatabaseRequest::new(DatabaseRequestVariant::Get(DatabaseItemID::UserData));
    data.database.send_prio(request);
    let reply = recv.recv().unwrap();
    match reply.variant {
        DatabaseReplyVariant::ReturnedItem(DatabaseItem::UserData(user_data)) => {
            if user_data.password_hash == payload.password_hash && user_data.pseudonym == payload.username {
                let (request, recv) = DatabaseRequest::new(DatabaseRequestVariant::NewAuthKey);
                data.database.send_prio(request);
                match recv.recv().unwrap().variant {
                    DatabaseReplyVariant::NewAuth(new_auth) => {
                        let (request, recv) = DatabaseRequest::new(DatabaseRequestVariant::Info(DatabaseInfoRequest::NumbersOfItems));
                        data.database.send_prio(request);
                        match recv.recv().unwrap().variant {
                            DatabaseReplyVariant::Info(DatabaseInfoReply::NumbersOfItems { devices, chats, folders, files, tags, access_modes }) => {
                                let mut found_device = false;
                                for i in 0..devices {
                                    let (request, recv) = DatabaseRequest::new(DatabaseRequestVariant::Get(DatabaseItemID::Device(i)));
                                    data.database.send_normal(request);
                                    match recv.recv().unwrap().variant {
                                        DatabaseReplyVariant::ReturnedItem(DatabaseItem::Device(device)) => {
                                            if &device.device_model == &payload.device_model && &device.device_name == &payload.device_name && &device.device_type == &payload.device_type {
                                                found_device = true;
                                                break
                                            }
                                        },
                                        _ => panic!("Confusion on return")
                                    }
                                }
                                if !found_device {
                                    let (request, recv) = DatabaseRequest::new(DatabaseRequestVariant::Add(DatabaseItem::Device(Device::new(0, payload.device_name.clone(), payload.device_type.clone(), payload.device_os.clone(), payload.device_model.clone()))));
                                    data.database.send_normal(request);
                                    match recv.recv().unwrap().variant {
                                        DatabaseReplyVariant::RequestExecuted => (),
                                        _ => panic!("Confusion on return")
                                    }
                                }
                            },
                            _ => panic!("Confusion on return")
                        }
                        HttpResponse::Ok().json(AuthResponse {  
                            session_token:new_auth, 
                        })  
                    },
                    _ => panic!("Confusion on return")
                } 
                
            }
            else {
                HttpResponse::Forbidden().json("Wrong username or password")
            }
        },
        _ => panic!("Confusion on return")
    }
    
}

pub fn is_auth_right(auth:String, data: web::Data<Arc<ProximaHandler>>) -> bool {
    let (request, recv) = DatabaseRequest::new(DatabaseRequestVariant::VerifyAuthKey(auth));
    data.database.send_prio(request);
    match recv.recv().unwrap().variant {
        DatabaseReplyVariant::CorrectAuth => true,
        DatabaseReplyVariant::WrongAuth => false,
        _ => panic!("Wrong return")
    }
}