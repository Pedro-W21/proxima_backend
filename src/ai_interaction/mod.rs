use std::{sync::mpmc::{channel, Receiver, Sender}, thread};

use backend_api::BackendAPI;
use endpoint_api::{EndpointRequest, EndpointRequestVariant, EndpointResponse, EndpointResponseVariant};

use crate::database::{chats::SessionType, DatabaseRequest, DatabaseSender};

pub mod endpoint_api;
pub mod ai_response;
pub mod backend_api;
pub mod create_prompt;

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
    pub fn respond(mut self) {
        match self.request_variant {
            EndpointRequestVariant::Continue => (),
            EndpointRequestVariant::RespondToFullPrompt { whole_context, streaming, session_type } => {
                let id = self.backend.send_new_prompt(whole_context, session_type);
                let response = self.backend.get_response_to_latest_prompt_for_blocking(id);
                self.response_sender.send(EndpointResponse { variant: EndpointResponseVariant::Block(response) }).unwrap();
            }
        }
    }
    pub fn streaming_respond(mut self) {
        match self.request_variant {
            EndpointRequestVariant::Continue => (),
            EndpointRequestVariant::RespondToFullPrompt { whole_context, streaming, session_type } => {
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

impl<B:BackendAPI + 'static> AIEndpoint<B> {
    pub fn new(prio_requests:Receiver<EndpointRequest>, requests:Receiver<EndpointRequest>, conn_data:B::ConnData, database_sender:DatabaseSender) -> Self {
        Self { backend_conn: conn_data, database_sender, prio_requests, requests }
    }
    pub fn handling_loop(mut self) {
        loop {
            match self.prio_requests.recv() {
                Ok(request) => {
                    self.handle_request(request);
                },
                Err(error) => panic!("Database access error : {}", error)
            }
            loop {
                if self.prio_requests.is_empty() {
                    if let Ok(request) = self.requests.try_recv() {
                        self.handle_request(request);
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
    pub fn handle_request(&mut self, request:EndpointRequest) {
        match request.variant.clone() {
            EndpointRequestVariant::Continue => (),
            EndpointRequestVariant::RespondToFullPrompt { whole_context, streaming, session_type } => {
                if streaming {
                    let db_send_clone = self.database_sender.clone();
                    let response = request.response_tunnel.clone();
                    let request = request.variant.clone();
                    let backend_conn = self.backend_conn.clone();
                    thread::spawn(move || {
                        RequestHandler::new(db_send_clone, request, response, B::new(backend_conn), streaming).streaming_respond();
                    });
                }
                else {
                    let db_send_clone = self.database_sender.clone();
                    let response = request.response_tunnel.clone();
                    let request = request.variant.clone();
                    let backend_conn = self.backend_conn.clone();
                    thread::spawn(move || {
                        RequestHandler::new(db_send_clone, request, response, B::new(backend_conn), streaming).respond();
                    });
                }
            }
        }
    }
}

pub fn launch_ai_endpoint_thread<B:BackendAPI + 'static>(conn_data:B::ConnData,database_sender:DatabaseSender, prio_send:Sender<EndpointRequest>, prio_rcv:Receiver<EndpointRequest>, normal_send:Sender<EndpointRequest>, normal_rcv:Receiver<EndpointRequest>) -> AiEndpointSender {
    let conn_copy = conn_data.clone();
    thread::spawn(move || {
        AIEndpoint::<B>::new(prio_rcv, normal_rcv, conn_copy, database_sender).handling_loop();
    });
    AiEndpointSender { prio_request_sender:prio_send, request_sender:normal_send }
}