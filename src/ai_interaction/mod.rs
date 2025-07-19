use std::{sync::mpmc::{channel, Receiver, Sender}, thread::{self, JoinHandle}};

use backend_api::BackendAPI;
use endpoint_api::{EndpointRequest, EndpointRequestVariant, EndpointResponse, EndpointResponseVariant};

use crate::{ai_interaction::tools::{handle_tool_calling_response, is_valid_tool_calling_response}, database::{chats::SessionType, DatabaseRequest, DatabaseSender}};

pub mod endpoint_api;
pub mod ai_response;
pub mod backend_api;
pub mod create_prompt;
pub mod tools;

pub struct AIEndpoint<B:BackendAPI> {
    backend_conn:B::ConnData,
    database_sender:DatabaseSender,
    prio_requests:Receiver<EndpointRequest>,
    requests:Receiver<EndpointRequest>
}

pub struct RequestHandler<B:BackendAPI> {
    database_sender:DatabaseSender,
    request_variant:EndpointRequestVariant,
    response_sender:Sender<EndpointResponse>,
    backend:B,
    streaming:bool
}

impl<B:BackendAPI> RequestHandler<B> {
    pub fn new(database_sender:DatabaseSender, request_variant:EndpointRequestVariant, response_sender:Sender<EndpointResponse>, backend:B, streaming:bool) -> Self {
        Self { database_sender, request_variant, response_sender, backend, streaming }
    }
    pub async fn respond(mut self) {
        match self.request_variant {
            EndpointRequestVariant::Continue => (),
            EndpointRequestVariant::RespondToFullPrompt { mut whole_context, streaming, session_type, chat_settings } => {
                match chat_settings {
                    Some(settings) => {
                        println!("in settings response cycle");
                        let id = self.backend.send_new_prompt(whole_context.clone(), session_type);
                        println!("Sent prompt !!!");
                        let mut response = self.backend.get_response_to_latest_prompt_for(id).await;
                        
                        match settings.get_tools() {
                            Some(tools) => {
                                let mut new_tools = tools.clone();
                                while !is_valid_tool_calling_response(&response) {
                                    let (added_context, output_tools) = handle_tool_calling_response(response.clone(), new_tools.clone());
                                    whole_context.add_part(response.clone());
                                    whole_context.add_part(added_context);
                                    whole_context.add_part(new_tools.get_tool_data_insert());
                                    new_tools = output_tools;
                                    println!("in settings response cycle");
                                    let id = self.backend.send_new_prompt(whole_context.clone(), session_type);
                                    println!("Sent prompt !!!");
                                    response = self.backend.get_response_to_latest_prompt_for(id).await;
                                }
                                whole_context.add_part(response);
                                println!("got response");
                                self.response_sender.send(EndpointResponse { variant: EndpointResponseVariant::MultiTurnBlock(whole_context) }).unwrap();
                                println!("Sent back response");
                            },
                            None => {
                                println!("got response");
                                self.response_sender.send(EndpointResponse { variant: EndpointResponseVariant::Block(response) }).unwrap();
                                println!("Sent back response");
                            },
                        }
                    },
                    None => {
                        println!("in no-setting response cycle");
                        let id = self.backend.send_new_prompt(whole_context, session_type);
                        println!("Sent prompt !!!");
                        let response = self.backend.get_response_to_latest_prompt_for(id).await;
                        println!("got response");
                        self.response_sender.send(EndpointResponse { variant: EndpointResponseVariant::Block(response) }).unwrap();
                        println!("Sent back response");
                    }
                }
                
            }
        }
    }
    pub async fn streaming_respond(mut self) {
        match self.request_variant {
            EndpointRequestVariant::Continue => (),
            EndpointRequestVariant::RespondToFullPrompt { whole_context, streaming, session_type, chat_settings } => {
                let id = self.backend.send_new_prompt(whole_context, session_type);
                let response = self.backend.get_response_to_latest_prompt_for_blocking(id);
            }
        }
    }
}

pub struct AiEndpointSender {
    prio_request_sender:Sender<EndpointRequest>,
    request_sender:Sender<EndpointRequest>
}

impl AiEndpointSender {
    pub fn send_prio(&self, request:EndpointRequest) {
        self.prio_request_sender.send(request);
    }
    pub fn send_normal(&self, request:EndpointRequest) {
        self.request_sender.send(request);
    }
}

impl<B:BackendAPI + Send + 'static> AIEndpoint<B> {
    pub fn new(prio_requests:Receiver<EndpointRequest>, requests:Receiver<EndpointRequest>, conn_data:B::ConnData, database_sender:DatabaseSender) -> Self {
        Self { backend_conn: conn_data, database_sender, prio_requests, requests }
    }
    pub async fn handling_loop(mut self) {
        loop {
            match self.prio_requests.recv() {
                Ok(request) => {
                    self.handle_request(request).await;
                },
                Err(error) => panic!("Database access error : {}", error)
            }
            loop {
                if self.prio_requests.is_empty() {
                    if let Ok(request) = self.requests.try_recv() {
                        self.handle_request(request).await;
                    }
                    else {
                        break;
                    }
                }
                else {
                    break;
                }
            }
        }
    }
    pub async fn handle_request(&mut self, request:EndpointRequest) {
        match request.variant.clone() {
            EndpointRequestVariant::Continue => (),
            EndpointRequestVariant::RespondToFullPrompt { whole_context, streaming, session_type, chat_settings} => {
                if streaming {
                    let db_send_clone = self.database_sender.clone();
                    let response = request.response_tunnel.clone();
                    let request = request.variant.clone();
                    let backend_conn = self.backend_conn.clone();
                    thread::spawn( move || async move {
                        RequestHandler::new(db_send_clone, request, response, B::new(backend_conn), streaming).streaming_respond();
                    });
                }
                else {
                    let db_send_clone = self.database_sender.clone();
                    let response = request.response_tunnel.clone();
                    let request = request.variant.clone();
                    let backend_conn = self.backend_conn.clone();
                    let thread = thread::spawn( move || async move {
                        RequestHandler::new(db_send_clone, request, response, B::new(backend_conn), streaming).respond().await;
                    });
                    let result = thread.join().unwrap().await;
                }
            }
        }
    }
}

pub async fn launch_ai_endpoint_thread<B:BackendAPI + Send + 'static>(conn_data:B::ConnData,database_sender:DatabaseSender, prio_send:Sender<EndpointRequest>, prio_rcv:Receiver<EndpointRequest>, normal_send:Sender<EndpointRequest>, normal_rcv:Receiver<EndpointRequest>) -> (AiEndpointSender, JoinHandle<impl Future<Output = ()>>) {
    let conn_copy = conn_data.clone();
    let ai_thread = thread::spawn(move || async move {
        AIEndpoint::<B>::new(prio_rcv, normal_rcv, conn_copy, database_sender).handling_loop().await;
    });
    (AiEndpointSender { prio_request_sender:prio_send, request_sender:normal_send }, ai_thread)
}