use std::{sync::{mpmc::{self, Receiver, Sender, channel}, mpsc::RecvTimeoutError}, thread::{self, JoinHandle}, time::Duration};

use backend_api::BackendAPI;
use endpoint_api::{EndpointRequest, EndpointRequestVariant, EndpointResponse, EndpointResponseVariant};

use crate::{ai_interaction::tools::{handle_tool_calling_response, is_valid_tool_calling_response, looks_like_nonstandard_final_response}, database::{DatabaseRequest, DatabaseSender, chats::SessionType, context::{ContextData, ContextPart, ContextPosition}}};

pub mod endpoint_api;
pub mod ai_response;
pub mod backend_api;
pub mod create_prompt;
pub mod tools;

pub struct AIEndpoint<B:BackendAPI> {
    backend_conn:B::ConnData,
    database_sender:DatabaseSender,
    prio_requests:Receiver<EndpointRequest>,
    requests:Receiver<EndpointRequest>,
    self_sender:AiEndpointSender
}

pub struct RequestHandler<B:BackendAPI> {
    database_sender:DatabaseSender,
    self_sender:AiEndpointSender,
    request_variant:EndpointRequestVariant,
    response_sender:Sender<EndpointResponse>,
    backend:B,
    streaming:bool
}

impl<B:BackendAPI> RequestHandler<B> {
    pub fn new(database_sender:DatabaseSender, request_variant:EndpointRequestVariant, response_sender:Sender<EndpointResponse>, backend:B, streaming:bool, self_sender:AiEndpointSender) -> Self {
        Self { database_sender, request_variant, response_sender, backend, streaming, self_sender }
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
                                while !is_valid_tool_calling_response(&response) && !looks_like_nonstandard_final_response(&response) && i < 8 {
                                    let (added_context, output_tools) = handle_tool_calling_response(response.clone(), new_tools.clone(), self.database_sender.clone(), self.self_sender.clone()).await;
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
                                if looks_like_nonstandard_final_response(&response) {
                                    response.get_data_mut().insert(0, ContextData::Text("<response>\n".to_string()));
                                    response.get_data_mut().push(ContextData::Text("</response>\n".to_string()));
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
                        send_streaming_response(receiver, ContextPosition::AI, self.response_sender.clone(), rep_sender.clone()).await;
                        let mut response = self.backend.get_response_to_latest_prompt_for(id).await;
                        loop {
                            match rep_recv.recv_timeout(Duration::from_millis(20)) {
                                Ok(resp) => {
                                    response = resp;
                                    break;
                                },
                                Err(error) => match error {
                                    RecvTimeoutError::Disconnected => break,
                                    RecvTimeoutError::Timeout => ()
                                }
                            }
                            special_bad_wait(80).await;
                        }
                        response.concatenate_text();
                        match settings.get_tools() {
                            Some(tools) => {
                                println!("Got tools, is tool calling response : {}, looks like nonstandard : {}", is_valid_tool_calling_response(&response), looks_like_nonstandard_final_response(&response));
                                let mut new_tools = tools.clone();
                                let mut i = 0;
                                while !is_valid_tool_calling_response(&response) && !looks_like_nonstandard_final_response(&response) && i < 8 {
                                    let (added_context, output_tools) = handle_tool_calling_response(response.clone(), new_tools.clone(), self.database_sender.clone(), self.self_sender.clone()).await;
                                    whole_context.add_part(response.clone());
                                    send_context_part_streaming_blocking(added_context.clone(), self.response_sender.clone());
                                    whole_context.add_part(added_context);
                                    send_context_part_streaming_blocking(new_tools.get_tool_data_insert(), self.response_sender.clone());
                                    whole_context.add_part(new_tools.get_tool_data_insert());
                                    new_tools = output_tools;
                                    println!("in settings response cycle");
                                    let (id, receiver) = self.backend.send_new_prompt_streaming(whole_context.clone(), session_type, Some(settings.clone()));

                                    send_streaming_response(receiver, ContextPosition::AI, self.response_sender.clone(), rep_sender.clone()).await;
                                    println!("Sent prompt !!!");
                                    response = self.backend.get_response_to_latest_prompt_for(id).await;
                                    loop {
                                        match rep_recv.recv_timeout(Duration::from_millis(20)) {
                                            Ok(resp) => {
                                                response = resp;
                                                break;
                                            },
                                            Err(error) => match error {
                                                RecvTimeoutError::Disconnected => break,
                                                RecvTimeoutError::Timeout => ()
                                            }
                                        }
                                        special_bad_wait(80).await;
                                    }
                                    response.concatenate_text();
                                    i += 1;
                                }

                                if looks_like_nonstandard_final_response(&response) {
                                    response.get_data_mut().insert(0, ContextData::Text("<response>\n".to_string()));
                                    response.get_data_mut().push(ContextData::Text("</response>\n".to_string()));
                                }
                                whole_context.add_part(response);
                                println!("got response");
                                // self.response_sender.send(EndpointResponse { variant: EndpointResponseVariant::MultiTurnBlock(whole_context) }).unwrap();
                                println!("Sent back response");
                            },
                            None => {
                                println!("Got no tools");
                                println!("got response");
                                // self.response_sender.send(EndpointResponse { variant: EndpointResponseVariant::Block(response) }).unwrap();
                                println!("Sent back response");
                            },
                        }
                    },
                    None => {
                        println!("in no-setting response cycle");
                        let (id, receiver) = self.backend.send_new_prompt_streaming(whole_context, session_type, None);
                        println!("Preparing streaming response");
                        send_streaming_response(receiver, ContextPosition::AI, self.response_sender.clone(), rep_sender.clone()).await;
                        println!("sending streaming response");
                        let response = self.backend.get_response_to_latest_prompt_for(id).await;
                        println!("Sent back response");
                    }
                }
            }
        }
    }
    
}

#[cfg(not(target_family = "wasm"))]
async fn send_streaming_response(receiver:Receiver<ContextData>, position:ContextPosition, sender:Sender<EndpointResponse>, total_sender:Sender<ContextPart>) {
    tokio::spawn(async move  {
        let mut total = ContextPart::new(vec![], position.clone());

        println!("[streaming response] Waiting on first token");
        loop {
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(data) => {
                    total.add_data(data.clone());
                    sender.send(EndpointResponse { variant: EndpointResponseVariant::StartStream(data, position.clone()) });
                    break;
                },
                Err(error) => ()
            }
            special_bad_wait(50).await;
        }
        

        println!("[streaming response] Got first token");
        loop {
            match receiver.recv_timeout(Duration::from_millis(10)) {
                Ok(data) => {
                    total.add_data(data.clone());
                    sender.send(EndpointResponse { variant: EndpointResponseVariant::ContinueStream(data, position.clone()) });
                    println!("[streaming response] Passing on token");
                },
                Err(error) => match error {
                    RecvTimeoutError::Disconnected => break,
                    _ => ()
                }
            }
            special_bad_wait(5).await;
        }
        total_sender.send(total);
    });
}

#[cfg(all(target_family = "wasm"))]
async fn send_streaming_response(receiver:Receiver<ContextData>, position:ContextPosition, sender:Sender<EndpointResponse>, total_sender:Sender<ContextPart>) {
    todo!("Support streaming responses on wasm")
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
#[derive(Clone)]
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
    pub fn new(prio_requests:Receiver<EndpointRequest>, requests:Receiver<EndpointRequest>, conn_data:B::ConnData, database_sender:DatabaseSender, self_sender:AiEndpointSender) -> Self {
        Self { backend_conn: conn_data, database_sender, prio_requests, requests, self_sender }
    }
    pub async fn handling_loop(mut self) {
        loop {
            match self.prio_requests.recv_timeout(Duration::from_millis(100)) {
                Ok(request) => {
                    handle_request::<B>(self.database_sender.clone(), self.backend_conn.clone(),request, self.self_sender.clone()).await;
                },
                Err(error) => match error {
                    RecvTimeoutError::Timeout => (),
                    _ => panic!("Database access error : {}", error)
                }
            }
            special_bad_wait(900).await;
            loop {
                if self.prio_requests.is_empty() {
                    if let Ok(request) = self.requests.try_recv() {
                        handle_request::<B>(self.database_sender.clone(), self.backend_conn.clone(),request, self.self_sender.clone()).await;
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
}

#[cfg(not(target_family = "wasm"))]
async fn special_bad_wait(millis:u64) {
    async_std::task::sleep(Duration::from_millis(millis)).await;
}

#[cfg(all(target_family = "wasm"))]
async fn special_bad_wait(millis:u64) {
    panic!("This function is not supported on WASM")
}


#[cfg(not(target_family = "wasm"))]
pub async fn handle_request<B:BackendAPI + Send + 'static>(db_sender:DatabaseSender, backend_conn:<B as BackendAPI>::ConnData, request:EndpointRequest, self_sender:AiEndpointSender) {
    println!("Handling request !");
    tokio::spawn(async move {
        println!("Inside task !");
        match request.variant.clone() {
            EndpointRequestVariant::Continue => (),
            EndpointRequestVariant::RespondToFullPrompt { whole_context, streaming, session_type, chat_settings} => {

                let response = request.response_tunnel.clone();
                let request = request.variant.clone();
                if streaming {
                    RequestHandler::new(db_sender, request, response, B::new(backend_conn), streaming, self_sender).streaming_respond().await;
                }
                else {
                    RequestHandler::new(db_sender, request, response, B::new(backend_conn), streaming, self_sender).respond().await;
                }
            }
        }
    });
}

#[cfg(all(target_family = "wasm"))]
pub async fn handle_request<B:BackendAPI>(db_sender:DatabaseSender, backend_conn:<B as BackendAPI>::ConnData, request:EndpointRequest, self_sender:AiEndpointSender) {
    match request.variant.clone() {
        EndpointRequestVariant::Continue => (),
        EndpointRequestVariant::RespondToFullPrompt { whole_context, streaming, session_type, chat_settings} => {

            let response = request.response_tunnel.clone();
            let request = request.variant.clone();
            if streaming {
                RequestHandler::new(db_sender, request, response, B::new(backend_conn), streaming, self_sender).streaming_respond().await;
            }
            else {
                RequestHandler::new(db_sender, request, response, B::new(backend_conn), streaming, self_sender).respond().await;
            }
        }
    }
}

pub async fn launch_ai_endpoint_thread<B:BackendAPI + Send + 'static>(conn_data:B::ConnData,database_sender:DatabaseSender, prio_send:Sender<EndpointRequest>, prio_rcv:Receiver<EndpointRequest>, normal_send:Sender<EndpointRequest>, normal_rcv:Receiver<EndpointRequest>) -> (AiEndpointSender, JoinHandle<impl Future<Output = ()>>) {
    let conn_copy = conn_data.clone();
    let ai_endpoint_sender = AiEndpointSender { prio_request_sender:prio_send, request_sender:normal_send };
    let ai_sender_clone= ai_endpoint_sender.clone();
    let ai_thread = thread::spawn(move || async move {
        AIEndpoint::<B>::new(prio_rcv, normal_rcv, conn_copy, database_sender, ai_sender_clone).handling_loop().await;
    });
    (ai_endpoint_sender, ai_thread)
}