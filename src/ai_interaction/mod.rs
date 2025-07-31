use std::{sync::mpmc::{self, channel, Receiver, Sender}, thread::{self, JoinHandle}};

use backend_api::BackendAPI;
use endpoint_api::{EndpointRequest, EndpointRequestVariant, EndpointResponse, EndpointResponseVariant};

use crate::{ai_interaction::tools::{handle_tool_calling_response, is_valid_tool_calling_response}, database::{chats::SessionType, context::{ContextData, ContextPart, ContextPosition}, DatabaseRequest, DatabaseSender}};

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
                        let id = self.backend.send_new_prompt(whole_context.clone(), session_type, Some(settings.clone()));
                        println!("Sent prompt !!!");
                        let mut response = self.backend.get_response_to_latest_prompt_for(id).await;
                        
                        match settings.get_tools() {
                            Some(tools) => {
                                let mut new_tools = tools.clone();
                                let mut i = 0;
                                while !is_valid_tool_calling_response(&response) && i < 8 {
                                    let (added_context, output_tools) = handle_tool_calling_response(response.clone(), new_tools.clone());
                                    whole_context.add_part(response.clone());
                                    whole_context.add_part(added_context);
                                    whole_context.add_part(new_tools.get_tool_data_insert());
                                    new_tools = output_tools;
                                    println!("in settings response cycle");
                                    let id = self.backend.send_new_prompt(whole_context.clone(), session_type, Some(settings.clone()));
                                    println!("Sent prompt !!!");
                                    response = self.backend.get_response_to_latest_prompt_for(id).await;
                                    i += 1;
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
                        let id = self.backend.send_new_prompt(whole_context, session_type, None);
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
            EndpointRequestVariant::RespondToFullPrompt { mut whole_context, streaming, session_type, chat_settings } => {
                let (rep_sender, rep_recv) = mpmc::channel();
                match chat_settings {
                    Some(settings) => {
                        println!("in settings response cycle");
                        let (id, receiver) = self.backend.send_new_prompt_streaming(whole_context.clone(), session_type, Some(settings.clone()));
                        println!("Sent prompt !!!");
                        send_streaming_response(receiver, ContextPosition::AI, self.response_sender.clone(), rep_sender.clone());
                        let mut response = self.backend.get_response_to_latest_prompt_for(id).await;
                        response = rep_recv.recv().unwrap();
                        match settings.get_tools() {
                            Some(tools) => {
                                let mut new_tools = tools.clone();
                                let mut i = 0;
                                while !is_valid_tool_calling_response(&response) && i < 8 {
                                    let (added_context, output_tools) = handle_tool_calling_response(response.clone(), new_tools.clone());
                                    whole_context.add_part(response.clone());
                                    send_context_part_streaming_blocking(added_context.clone(), self.response_sender.clone());
                                    whole_context.add_part(added_context);
                                    send_context_part_streaming_blocking(new_tools.get_tool_data_insert(), self.response_sender.clone());
                                    whole_context.add_part(new_tools.get_tool_data_insert());
                                    new_tools = output_tools;
                                    println!("in settings response cycle");
                                    let (id, receiver) = self.backend.send_new_prompt_streaming(whole_context.clone(), session_type, Some(settings.clone()));

                                    send_streaming_response(receiver, ContextPosition::AI, self.response_sender.clone(), rep_sender.clone());
                                    println!("Sent prompt !!!");
                                    response = self.backend.get_response_to_latest_prompt_for(id).await;
                                    response = rep_recv.recv().unwrap();
                                    i += 1;
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
                        let (id, receiver) = self.backend.send_new_prompt_streaming(whole_context, session_type, None);
                        println!("Preparing streaming response");
                        send_streaming_response(receiver, ContextPosition::AI, self.response_sender.clone(), rep_sender.clone());
                        println!("sending streaming response");
                        let response = self.backend.get_response_to_latest_prompt_for(id).await;
                        println!("Sent back response");
                    }
                }
            }
        }
    }
    
}

fn send_streaming_response(receiver:Receiver<ContextData>, position:ContextPosition, sender:Sender<EndpointResponse>, total_sender:Sender<ContextPart>) {
    thread::spawn(move || {
        let mut total = ContextPart::new(vec![], position.clone());
        match receiver.recv() {
            Ok(data) => {
                total.add_data(data.clone());
                sender.send(EndpointResponse { variant: EndpointResponseVariant::StartStream(data, position.clone()) });
            },
            Err(error) => ()
        }
        loop {
            match receiver.recv() {
                Ok(data) => {
                    total.add_data(data.clone());
                    sender.send(EndpointResponse { variant: EndpointResponseVariant::ContinueStream(data, position.clone()) });
                },
                Err(error) => break
            }
        }
        total_sender.send(total);
    });
}

fn send_context_part_streaming_blocking(part:ContextPart, sender:Sender<EndpointResponse>) {
    let mut first = true;
    for data in part.get_data() {
        if first {
            sender.send(EndpointResponse { variant: EndpointResponseVariant::StartStream(data.clone(), part.get_position().clone()) });
            first = false;
        }
        else {
            sender.send(EndpointResponse { variant: EndpointResponseVariant::ContinueStream(data.clone(), part.get_position().clone()) });
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
                    let thread = thread::spawn( move || async move {
                        RequestHandler::new(db_send_clone, request, response, B::new(backend_conn), streaming).streaming_respond().await;
                    });
                    let result = thread.join().unwrap().await;
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