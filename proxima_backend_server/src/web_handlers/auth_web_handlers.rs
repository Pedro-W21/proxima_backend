use std::sync::Arc;

use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

use proxima_backend::{database::{DatabaseInfoReply, DatabaseInfoRequest, DatabaseItem, DatabaseItemID, DatabaseReplyVariant, DatabaseRequest, DatabaseRequestVariant, devices::{Device, DeviceType}, user::data_into_base64_hash}, proxima_handler::ProximaHandler};


use proxima_backend::web_payloads::{AuthPayload, AuthResponse};

pub async fn auth_post_handler(payload: web::Json<AuthPayload>, data: web::Data<Arc<ProximaHandler>>) -> impl Responder {
    // process payload and use handler
    let (request, recv) = DatabaseRequest::new(DatabaseRequestVariant::Get(DatabaseItemID::UserData), None);
    data.database.send_prio(request);
    println!("[authentication] Sent first DB request");
    let reply = recv.recv().unwrap();
    println!("[authentication] Received first DB response");
    match reply.variant {
        DatabaseReplyVariant::ReturnedItem(DatabaseItem::UserData(user_data)) => {
            if user_data.password_hash == data_into_base64_hash(payload.password.as_bytes().to_vec()) && user_data.pseudonym == payload.username {
                let (request, recv) = DatabaseRequest::new(DatabaseRequestVariant::NewAuthKey, None);
                data.database.send_prio(request);
                println!("[authentication] Sent second DB request"); 
                match recv.recv().unwrap().variant {
                    DatabaseReplyVariant::NewAuth(new_auth) => {
                        println!("[authentication] Received second DB response");
                        let mut device_id = 0;
                        let (request, recv) = DatabaseRequest::new(DatabaseRequestVariant::Info(DatabaseInfoRequest::NumbersOfItems), None);
                        data.database.send_prio(request);
                        println!("[authentication] Sent third DB request"); 
                        match recv.recv().unwrap().variant {
                            DatabaseReplyVariant::Info(DatabaseInfoReply::NumbersOfItems { devices, chats, folders, files, tags, access_modes }) => {
                                println!("[authentication] Received third DB response");
                                let mut found_device = false;
                                for i in 0..devices {
                                    let (request, recv) = DatabaseRequest::new(DatabaseRequestVariant::Get(DatabaseItemID::Device(i)), None);
                                    data.database.send_prio(request);
                                    match recv.recv().unwrap().variant {
                                        DatabaseReplyVariant::ReturnedItem(DatabaseItem::Device(device)) => {
                                            if &device.device_model == &payload.device_model && &device.device_name == &payload.device_name && &device.device_type == &payload.device_type {
                                                found_device = true;
                                                device_id = i;
                                                break
                                            }
                                        },
                                        _ => panic!("Confusion on return")
                                    }
                                }
                                if !found_device {
                                    let (request, recv) = DatabaseRequest::new(DatabaseRequestVariant::Add(DatabaseItem::Device(Device::new(0, payload.device_name.clone(), payload.device_type.clone(), payload.device_os.clone(), payload.device_model.clone()))), None);
                                    data.database.send_prio(request);
                                    device_id = match recv.recv().unwrap().variant {
                                        DatabaseReplyVariant::AddedItem(DatabaseItemID::Device(id)) => id,
                                        _ => panic!("Confusion on return")
                                    }
                                }
                            },
                            _ => panic!("Confusion on return")
                        }

                        println!("[authentication] Successfully authenticated, sending session token"); 
                        HttpResponse::Ok().json(AuthResponse {  
                            session_token:new_auth,
                            device_id
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
    let (request, recv) = DatabaseRequest::new(DatabaseRequestVariant::VerifyAuthKey(auth), None);
    data.database.send_prio(request);
    match recv.recv().unwrap().variant {
        DatabaseReplyVariant::CorrectAuth => true,
        DatabaseReplyVariant::WrongAuth => false,
        _ => panic!("Wrong return")
    }
}