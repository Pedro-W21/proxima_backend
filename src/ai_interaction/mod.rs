use std::{collections::HashSet, sync::{mpmc::{self, Receiver, Sender, channel}, mpsc::RecvTimeoutError}, thread::{self, JoinHandle}, time::Duration};

use backend_api::BackendAPI;
use endpoint_api::{EndpointRequest, EndpointRequestVariant, EndpointResponse, EndpointResponseVariant};

use crate::{ai_interaction::{backend_api::BackendError, tools::{ProximaTool, RuntimeToolData, bad_async_recv, handle_tool_calling_response, is_valid_tool_calling_response, looks_like_nonstandard_final_response}}, database::{DatabaseItem, DatabaseItemID, DatabaseReply, DatabaseReplyVariant, DatabaseRequest, DatabaseRequestVariant, DatabaseSender, ToolRequest, access_modes::AccessModeID, chats::{ChatID, SessionType}, context::{ContextData, ContextPart, ContextPosition, ToolPart, ToolPartKind, WholeContext}, jobs::{Job, JobRepeat, JobTiming, JobType}, notifications::{Notification, NotificationReason}}};

use crate::ai_interaction::endpoint_api::EndpointError;
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
    self_sender:AiEndpointSender,
    runtime_tool_data:RuntimeToolData
}

pub struct RequestHandler<B:BackendAPI> {
    database_sender:DatabaseSender,
    self_sender:AiEndpointSender,
    request_variant:EndpointRequestVariant,
    response_sender:Sender<EndpointResponse>,
    backend:B,
    streaming:bool,
    runtime_tool_data:RuntimeToolData
}

impl<B:BackendAPI> RequestHandler<B> {
    pub fn new(database_sender:DatabaseSender, request_variant:EndpointRequestVariant, response_sender:Sender<EndpointResponse>, backend:B, streaming:bool, self_sender:AiEndpointSender, runtime_tool_data:RuntimeToolData) -> Self {
        Self { database_sender, request_variant, response_sender, backend, streaming, self_sender, runtime_tool_data }
    }
    async fn update_chat(&mut self, new_whole_context:WholeContext, chat_id:Option<ChatID>, access_mode:AccessModeID) {
        match chat_id {
            Some(id) => {
                let (db_req, db_recv) = DatabaseRequest::new(DatabaseRequestVariant::ToolRequest(crate::database::ToolRequest::UpdateExistingChatContext(id, new_whole_context)), None);
                self.database_sender.send_prio(db_req);
                let reply = bad_async_recv(db_recv).await;
                match reply.variant {
                    DatabaseReplyVariant::RequestExecuted => {
                        let (db_req, db_recv) = DatabaseRequest::new(DatabaseRequestVariant::Add(DatabaseItem::Notification(Notification::new(Some(DatabaseItemID::Chat(id)), HashSet::from([0, access_mode]), NotificationReason::ChatRoundFinished, None))), None);
                        self.database_sender.send_prio(db_req);
                        let reply = bad_async_recv(db_recv).await;
                        let (db_req, db_recv) = DatabaseRequest::new(DatabaseRequestVariant::Get(DatabaseItemID::Chat(id)), None);
                        self.database_sender.send_prio(db_req);
                        if let DatabaseReply { variant:DatabaseReplyVariant::ReturnedItem(DatabaseItem::Chat(chat)) } = bad_async_recv(db_recv).await {
                            if chat.get_title().is_none() {
                                let (db_req, db_recv) = DatabaseRequest::new(DatabaseRequestVariant::Add(DatabaseItem::Job(Job::new(JobTiming::ASAP, JobRepeat::No, JobType::Title(id), None, HashSet::from([0, access_mode])))), None);
                                self.database_sender.send_prio(db_req);
                                let reply = bad_async_recv(db_recv).await;
                            }
                            if chat.tags.len() == 0 {
                                let (db_req, db_recv) = DatabaseRequest::new(DatabaseRequestVariant::Add(DatabaseItem::Job(Job::new(JobTiming::ASAP, JobRepeat::No, JobType::Tag(DatabaseItemID::Chat(id)), None, HashSet::from([0, access_mode])))), None);
                                self.database_sender.send_prio(db_req);
                                let reply = bad_async_recv(db_recv).await;
                            }
                        }
                    },
                    _ => println!("[AI request handler] chat update failed")
                } 
            },
            None => ()
        }
    }
    pub async fn respond(mut self) -> Result<(), BackendError> {
        match self.request_variant.clone() {
            EndpointRequestVariant::Continue => todo!("Implement continues"),
            EndpointRequestVariant::RespondToFullPrompt { mut whole_context, streaming, session_type, chat_settings, chat_id, access_mode } => {
                match chat_settings {
                    Some(settings) => {
                        println!("in settings response cycle");

                        if let Some(tools) = settings.get_tools() && tools.has_automatic_memory() {
                            update_auto_memory(&mut whole_context, self.database_sender.clone(), access_mode).await;
                        }
                        let id = self.backend.send_new_prompt(whole_context.clone(), session_type, Some(settings.clone()), self.database_sender.clone())?;
                        println!("Sent prompt !!!");
                        let mut response = self.backend.get_response_to_latest_prompt_for(id).await;
                        
                        match settings.get_tools() {
                            Some(tools) => {
                                let mut new_tools = tools.clone();
                                let mut i = 0;
                                while !is_valid_tool_calling_response(&response) && !looks_like_nonstandard_final_response(&response) && i < 8 {
                                    let (added_context, output_tools) = handle_tool_calling_response(response.clone(), new_tools.clone(), self.database_sender.clone(), self.self_sender.clone(), &self.runtime_tool_data, access_mode).await;
                                    whole_context.add_part(response.clone());
                                    whole_context.add_part(added_context);
                                    for part in new_tools.get_tool_data_insert(ContextPosition::AI) {
                                        whole_context.add_part(part);
                                    }
                                    new_tools = output_tools;
                                    println!("in settings response cycle");
                                    if tools.has_automatic_memory() {
                                        update_auto_memory(&mut whole_context, self.database_sender.clone(), access_mode).await;
                                    }
                                    let id = self.backend.send_new_prompt(whole_context.clone(), session_type, Some(settings.clone()), self.database_sender.clone())?;
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
                                self.update_chat(whole_context.clone(), chat_id, access_mode).await;
                                self.response_sender.send(EndpointResponse { variant: EndpointResponseVariant::MultiTurnBlock(whole_context) });
                                println!("Sent back response");
                            },
                            None => {
                                println!("got response");
                                whole_context.add_part(response.clone());
                                self.update_chat(whole_context, chat_id, access_mode).await;
                                self.response_sender.send(EndpointResponse { variant: EndpointResponseVariant::Block(response) });
                                println!("Sent back response");
                            },
                        }
                    },
                    None => {
                        println!("in no-setting response cycle");
                        let id = self.backend.send_new_prompt(whole_context.clone(), session_type, None, self.database_sender.clone())?;
                        println!("Sent prompt !!!");
                        let response = self.backend.get_response_to_latest_prompt_for(id).await;
                        println!("got response");
                        whole_context.add_part(response.clone());
                        self.update_chat(whole_context, chat_id, access_mode).await;
                        self.response_sender.send(EndpointResponse { variant: EndpointResponseVariant::Block(response) });
                        println!("Sent back response");
                    }
                }
                Ok(())
            }
        }
    }
    pub async fn streaming_respond(mut self) -> Result<(), BackendError> {
        match self.request_variant.clone() {
            EndpointRequestVariant::Continue => todo!("implement streaming continues"),
            EndpointRequestVariant::RespondToFullPrompt { mut whole_context, streaming, session_type, chat_settings, chat_id, access_mode } => {
                let (rep_sender, rep_recv) = mpmc::channel();
                match chat_settings {
                    Some(settings) => {
                        println!("in settings response cycle");
                        if let Some(tools) = settings.get_tools() && tools.has_automatic_memory() {
                            update_auto_memory(&mut whole_context, self.database_sender.clone(), access_mode).await;
                        }
                        let (id, receiver) = self.backend.send_new_prompt_streaming(whole_context.clone(), session_type, Some(settings.clone()), self.database_sender.clone())?;
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
                                    let (added_context, output_tools) = handle_tool_calling_response(response.clone(), new_tools.clone(), self.database_sender.clone(), self.self_sender.clone(), &self.runtime_tool_data, access_mode).await;
                                    whole_context.add_part(response.clone());
                                    send_context_part_streaming_blocking(added_context.clone(), self.response_sender.clone());
                                    whole_context.add_part(added_context);
                                    for part in new_tools.get_tool_data_insert(ContextPosition::AI) {
                                        send_context_part_streaming_blocking(part.clone(), self.response_sender.clone());
                                        whole_context.add_part(part);
                                    }
                                    new_tools = output_tools;
                                    println!("in settings response cycle");
                                    if tools.has_automatic_memory() {
                                        update_auto_memory(&mut whole_context, self.database_sender.clone(), access_mode).await;
                                    }
                                    let (id, receiver) = self.backend.send_new_prompt_streaming(whole_context.clone(), session_type, Some(settings.clone()), self.database_sender.clone())?;

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
                                self.update_chat(whole_context, chat_id, access_mode).await;
                                println!("got response");
                                // self.response_sender.send(EndpointResponse { variant: EndpointResponseVariant::MultiTurnBlock(whole_context) }).unwrap();
                                println!("Sent back response");
                            },
                            None => {
                                println!("Got no tools");
                                println!("got response");

                                whole_context.add_part(response);
                                self.update_chat(whole_context, chat_id, access_mode).await;
                                // self.response_sender.send(EndpointResponse { variant: EndpointResponseVariant::Block(response) }).unwrap();
                                println!("Sent back response");
                            },
                        }
                    },
                    None => {
                        println!("in no-setting response cycle");
                        let (id, receiver) = self.backend.send_new_prompt_streaming(whole_context.clone(), session_type, None, self.database_sender.clone())?;
                        println!("Preparing streaming response");
                        send_streaming_response(receiver, ContextPosition::AI, self.response_sender.clone(), rep_sender.clone()).await;
                        println!("sending streaming response");
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
                        whole_context.add_part(response);
                        self.update_chat(whole_context, chat_id, access_mode).await;
                        println!("Sent back response");
                    }
                }
                Ok(())
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

async fn update_auto_memory(context:&mut WholeContext, db_sender:DatabaseSender, access_mode:AccessModeID) {
    for part in context.get_parts_mut() {
        if let ContextPosition::Tool(ToolPart { kind:ToolPartKind::DataInsert, related_tool:Some(ProximaTool::Memory) }) = part.get_position().clone() {
            part.get_data_mut().clear();
            let (db_req, db_recv) = DatabaseRequest::new(DatabaseRequestVariant::ToolRequest(ToolRequest::GetAutoMemoryFor(access_mode, 10)), None);
            db_sender.send_prio(db_req);

            let addition = match bad_async_recv(db_recv).await.variant {
                DatabaseReplyVariant::ReturnedManyItems(items) => {

                    let mut total = match &items[0] {
                        DatabaseItem::AccessMode(_) => format!("No persistent memory"),
                        DatabaseItem::Memory(_, txt) => txt.lines().enumerate().map(|(i, line)| {let out = format!("{i} {}\n", line); out}).collect::<Vec<String>>().concat(),
                        _ => panic!("Impossible")
                    };
                    if let Some(items) = items.get(1..) {
                        for item in items {
                            if let DatabaseItem::Memory(mem, txt) = item {
                                total += &format!("\n----\n{}\n{txt}\n", mem.add_date);
                            }
                        }
                    }
                    total
                },
                _ => format!("Database unaccessible for memory update")
            };
            part.add_data(ContextData::Text(format!("<automatic_memory>\n{addition}\n</automatic_memory>\n")));
            break;
        }
    }
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
    pub fn new(prio_requests:Receiver<EndpointRequest>, requests:Receiver<EndpointRequest>, conn_data:B::ConnData, database_sender:DatabaseSender, self_sender:AiEndpointSender, runtime_tool_data:RuntimeToolData) -> Self {
        Self { backend_conn: conn_data, database_sender, prio_requests, requests, self_sender, runtime_tool_data }
    }
    pub async fn handling_loop(mut self) {
        loop {
            match self.prio_requests.recv_timeout(Duration::from_millis(100)) {
                Ok(request) => {
                    handle_request::<B>(self.database_sender.clone(), self.backend_conn.clone(),request, self.self_sender.clone(), self.runtime_tool_data.clone()).await;
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
                        handle_request::<B>(self.database_sender.clone(), self.backend_conn.clone(),request, self.self_sender.clone(), self.runtime_tool_data.clone()).await;
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
pub async fn handle_request<B:BackendAPI + Send + 'static>(db_sender:DatabaseSender, backend_conn:<B as BackendAPI>::ConnData, request:EndpointRequest, self_sender:AiEndpointSender, runtime_tool_data:RuntimeToolData) {
    println!("Handling request !");
    tokio::spawn(async move {
        println!("Inside task !");
        match request.variant.clone() {
            EndpointRequestVariant::Continue => (),
            EndpointRequestVariant::RespondToFullPrompt { whole_context, streaming, session_type, chat_settings, chat_id, access_mode} => {

                let response = request.response_tunnel.clone();
                let request = request.variant.clone();
                let value = if streaming {
                    RequestHandler::new(db_sender, request, response.clone(), B::new(backend_conn), streaming, self_sender, runtime_tool_data).streaming_respond().await
                }
                else {
                    RequestHandler::new(db_sender, request, response.clone(), B::new(backend_conn), streaming, self_sender, runtime_tool_data).respond().await
                };
                value.unwrap_or_else(|error| {
                    match error {
                        BackendError::BackendUnavailable => response.send(EndpointResponse { variant: EndpointResponseVariant::EndpointError(EndpointError::BackendUnavailable { url: String::from("don't have url") }) }).unwrap(),
                        _ => panic!("Other types of errors not possible here"),
                    }
                });
            }
        }
    });
}

#[cfg(all(target_family = "wasm"))]
pub async fn handle_request<B:BackendAPI>(db_sender:DatabaseSender, backend_conn:<B as BackendAPI>::ConnData, request:EndpointRequest, self_sender:AiEndpointSender, runtime_tool_data:RuntimeToolData) {
    panic!("Not implemented in WASM")
}

pub async fn launch_ai_endpoint_thread<B:BackendAPI + Send + 'static>(conn_data:B::ConnData,database_sender:DatabaseSender, prio_send:Sender<EndpointRequest>, prio_rcv:Receiver<EndpointRequest>, normal_send:Sender<EndpointRequest>, normal_rcv:Receiver<EndpointRequest>, runtime_tool_data:RuntimeToolData) -> (AiEndpointSender, JoinHandle<impl Future<Output = ()>>) {
    let conn_copy = conn_data.clone();
    let ai_endpoint_sender = AiEndpointSender { prio_request_sender:prio_send, request_sender:normal_send };
    let ai_sender_clone= ai_endpoint_sender.clone();
    let ai_thread = thread::spawn(move || async move {
        AIEndpoint::<B>::new(prio_rcv, normal_rcv, conn_copy, database_sender, ai_sender_clone, runtime_tool_data).handling_loop().await;
    });
    (ai_endpoint_sender, ai_thread)
}