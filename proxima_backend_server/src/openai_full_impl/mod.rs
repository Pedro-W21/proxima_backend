use std::{collections::HashMap, future::Future, pin::Pin, sync::{Arc, RwLock, mpmc::{Receiver, Sender, channel}}, thread, time::Duration};

use actix_web::rt::time::sleep;
use futures::StreamExt;
use openai_api_rs::v1::{api::OpenAIClient, chat_completion::{ChatCompletionChoice, ChatCompletionMessage, Content, MessageRole, chat_completion::{ChatCompletionRequest, ChatCompletionResponse}, chat_completion_stream::{ChatCompletionStreamRequest, ChatCompletionStreamResponse}}, common::Usage, error::APIError};
use proxima_backend::database::{configuration::ChatConfiguration, context::{ContextData, ContextPart, ContextPosition, Prompt, Response, WholeContext}};
use proxima_backend::database::chats::{SessionID, SessionType};


use proxima_backend::ai_interaction::backend_api::{BackendAPI, BackendError};

#[derive(Clone)]
pub struct OpenAIFullBackend {
    api_key:ApiKey,
    url:ChosenUrl,
    model:ChosenModel,
    sessions:HashMap<SessionID, OpenAISession>,
    latest_session_id:usize,
    total_tasks:usize,
    tasks:Arc<RwLock<Vec<Pin<Box<dyn Future<Output = ()> + Send + Sync>>>>>,
    task_sender:Sender<(Result<ChatCompletionResponse, APIError>, SessionID)>,
    results_recv:Receiver<(Result<ChatCompletionResponse, APIError>, SessionID)>
}
#[derive(Clone)]
pub enum OpenAISessionData {
    ChatComp{messages:Vec<ChatCompletionMessage>, context_ver:WholeContext, waiting_on:Option<Response>}
}

impl OpenAISessionData {
    pub fn get_context(&self) -> WholeContext {
        match self {
            Self::ChatComp { messages, context_ver, waiting_on } => context_ver.clone()
        }
    }
    pub fn get_response(&self) -> Option<Response> {
        match self {
            Self::ChatComp { messages, context_ver, waiting_on } => waiting_on.clone()
        }
    }
}
#[derive(Clone)]
pub enum OpenAISessionStatus {
    Beginning,
    Waiting,
    Standby,
    Over
}

impl OpenAISessionStatus {
    pub fn ready(&self) -> bool {
        match self {
            OpenAISessionStatus::Standby => true,
            _ => false
        }
    }
}
#[derive(Clone)]
pub struct OpenAISession {
    session_data:OpenAISessionData,
    status:OpenAISessionStatus
}

pub type ChosenModel = String;
pub type ChosenUrl = String;
pub type ApiKey = String;

impl BackendAPI for OpenAIFullBackend {
    type ConnData = (ChosenUrl, ApiKey, ChosenModel);
    fn new(connection_data:Self::ConnData) -> Self {
        let (send, recv) = channel();
        Self { api_key:connection_data.1, url:connection_data.0, model:connection_data.2, sessions:HashMap::with_capacity(16), latest_session_id:0, tasks:Arc::new(RwLock::new(Vec::new())), task_sender:send, results_recv:recv, total_tasks:0}
    }
    fn send_new_prompt_streaming(&mut self, new_prompt:WholeContext, session_type:SessionType, config:Option<ChatConfiguration>) -> (SessionID, Receiver<ContextData>) {
        let new_session_id = self.latest_session_id;
        self.latest_session_id += 1;
        let session_id = SessionID { id: new_session_id, session_type };
        let mut messages = Vec::with_capacity(new_prompt.len());
        for part in new_prompt.get_parts() {
            // Only supports text for now
            let mut final_content = String::new();
            for data in part.get_data() {
                match data {
                    ContextData::Text(text) => final_content.push_str(text.as_str()),
                    _ => panic!("Not implemented")
                }
            }
            match part.get_position() {
                ContextPosition::User => {
                    messages.push(ChatCompletionMessage { role: MessageRole::user, content: Content::Text(final_content), name:None, tool_calls:None, tool_call_id:None });
                },
                ContextPosition::System => {
                    messages.push(ChatCompletionMessage { role: MessageRole::system, content: Content::Text(final_content), name:None, tool_calls:None, tool_call_id:None });
                },
                ContextPosition::AI => {
                    messages.push(ChatCompletionMessage { role: MessageRole::assistant, content: Content::Text(final_content), name:None, tool_calls:None, tool_call_id:None });
                },
                ContextPosition::Total | ContextPosition::Tool => {
                    messages.push(ChatCompletionMessage { role: MessageRole::tool, content: Content::Text(final_content), name:None, tool_calls:None, tool_call_id:None });
                }
            }
        }
        let messages_clones = messages.clone();
        let session_clones = session_id.clone();
        let model_clone = self.model.clone();
        let mut client = OpenAIClient::builder()
            .with_endpoint(self.url.clone())
            .with_api_key(self.api_key.clone())
            .build().unwrap();
        let sender_clone = self.task_sender.clone();
        let (sender_to_client, receiver_for_client) = channel();
        tokio::spawn(async move {

            sender_clone.send((Ok(ChatCompletionResponse { id: None, object: None, created: 100, model: String::from("AAAA"), choices: vec![], usage: Usage {prompt_tokens:1, completion_tokens:1, total_tokens:2}, system_fingerprint: None }), session_clones)).unwrap();
            let mut receiver =
            match config {
                Some(config) => 
                {
                    let request = ChatCompletionStreamRequest::new(model_clone, messages_clones)
                        .max_tokens(config.get_max_response() as i64)
                        .temperature(config.get_temp() as f64);
                    client.chat_completion_stream(request).await.unwrap()
                }
                None => {
                    let request = ChatCompletionStreamRequest::new(model_clone, messages_clones);
                    client.chat_completion_stream(request).await.unwrap()
                }
            };
            let mut total = Vec::new();
            loop {
                match receiver.next().await {
                    Some(completion) => {
                        total.push(completion.clone());
                        match completion {
                            ChatCompletionStreamResponse::Content(content) => {
                                sender_to_client.send(ContextData::Text(content.clone()));
                            },
                            ChatCompletionStreamResponse::Done => break,
                            _ => ()
                        }
                    },
                    None => break,
                }
            }
        
        });
        let completion = Box::pin( (async move || {
            
        })());
        {
            self.tasks.write().unwrap().push(completion);
            self.total_tasks += 1;
        }
        self.sessions.insert(session_id, OpenAISession { session_data: OpenAISessionData::ChatComp { messages: messages, context_ver: new_prompt, waiting_on:None }, status: OpenAISessionStatus::Beginning });
        (session_id, receiver_for_client)
    }
    fn new_empty() -> Self {
        let (send, recv) = channel();
        Self { api_key:String::new(), url:String::new(), model:String::new(), sessions:HashMap::with_capacity(16), latest_session_id:0, tasks:Arc::new(RwLock::new(Vec::new())), task_sender:send, results_recv:recv, total_tasks:0}
    
    }
    async fn get_response_to_latest_prompt_for(&mut self, session:SessionID) -> Response {
        'a: while self.total_tasks > 0 {
                {
                    self.total_tasks -= 1;
                    let future = {
                        let mut task_write = self.tasks.write().unwrap();
                        task_write.remove(0)
                    }; 
                    future.await;
                }
                
                loop {
                    if let Ok((result, session_id)) = self.results_recv.recv_timeout(Duration::from_millis(10)) {
                        let response = result.unwrap();
                        let completion = if response.choices.len() > 0 {
                            let msg = response.choices[0].clone().message;
                            msg.content.clone()
                        }
                        else {
                            Some(format!(" "))
                        };
                        match self.sessions.get_mut(&session_id) {
                                Some(session_data) => {
                                    match &mut session_data.session_data {
                                        OpenAISessionData::ChatComp { messages, context_ver, waiting_on } => {
                                            *waiting_on = completion.clone().and_then(|message| {context_ver.add_part(Response::new(vec![ContextData::Text(message.clone())], ContextPosition::AI));Some(Response::new(vec![ContextData::Text(message)], ContextPosition::AI)) });
                                            session_data.status = OpenAISessionStatus::Standby;
                                            messages.push(ChatCompletionMessage { role: MessageRole::assistant, content: completion.map_or(Content::Text("".to_string()), |value| {Content::Text(value)}), name: None, tool_calls: None, tool_call_id: None });
                                            if session_id == session {
                                                break 'a;
                                            }
                                        }
                                    }
                                }
                                None => ()
                            }
                    }
                    sleep(Duration::from_millis(90)).await;
                }
        }
        
        match self.sessions.get_mut(&session) {
            Some(sess) => {
                let value = sess.session_data.get_response().clone().unwrap();
                value
            },
            None => panic!("Session is supposed to exist")
        }

    }
    fn try_get_response_to_latest_prompt_for(&mut self, session:SessionID) -> Option<Response> {
        while let Ok((result, session_id)) = self.results_recv.try_recv() {
            let msg = result.unwrap().choices[0].clone().message;
            let completion = msg.content.clone();
            match self.sessions.get_mut(&session_id) {
                Some(session_data) => {
                    match &mut session_data.session_data {
                        OpenAISessionData::ChatComp { messages, context_ver, waiting_on } => {
                            *waiting_on = completion.and_then(|message| {context_ver.add_part(Response::new(vec![ContextData::Text(message.clone())], ContextPosition::AI));Some(Response::new(vec![ContextData::Text(message)], ContextPosition::AI)) });
                            session_data.status = OpenAISessionStatus::Standby;
                            messages.push(ChatCompletionMessage { role: MessageRole::assistant, content: msg.content.map_or(Content::Text("".to_string()), |value| {Content::Text(value)}), name: None, tool_calls: None, tool_call_id: None });
                        }
                    }
                }
                None => ()
            }
        }
        match self.sessions.get_mut(&session) {
            Some(sess) => {
                let value = sess.session_data.get_response().clone();
                value
            },
            None => None
        }
    }
    fn get_response_to_latest_prompt_for_blocking(&mut self, session:SessionID) -> Response {
        while let Ok((result, session_id)) = self.results_recv.recv() {
            let msg = result.unwrap().choices[0].clone().message;
            let completion = msg.content.clone();
            match self.sessions.get_mut(&session_id) {
                Some(session_data) => {
                    match &mut session_data.session_data {
                        OpenAISessionData::ChatComp { messages, context_ver, waiting_on } => {
                            *waiting_on = completion.and_then(|message| {context_ver.add_part(Response::new(vec![ContextData::Text(message.clone())], ContextPosition::AI));Some(Response::new(vec![ContextData::Text(message)], ContextPosition::AI)) });
                            session_data.status = OpenAISessionStatus::Standby;
                            messages.push(ChatCompletionMessage { role: MessageRole::assistant, content: msg.content.map_or(Content::Text("".to_string()), |value| {Content::Text(value)}), name: None, tool_calls: None, tool_call_id: None });
                        }
                    }
                }
                None => ()
            }
        }
        match self.sessions.get_mut(&session) {
            Some(sess) => {
                let value = sess.session_data.get_response().clone().unwrap();
                value
            },
            None => panic!("This session should exist ! {:?}", session),
        }
    }
    fn add_to_session(&mut self, new_prompt:Prompt, session:SessionID) -> Result<(), BackendError> {
        match self.sessions.get_mut(&session) {
            Some(session_data) => {
                if session_data.status.ready() {
                    match &mut session_data.session_data {
                        OpenAISessionData::ChatComp { messages, context_ver, waiting_on } => {
                            context_ver.add_part(new_prompt.clone());
                            *waiting_on = None;
                            session_data.status = OpenAISessionStatus::Waiting;
                            // Only supports text for now
                            let mut final_content = String::new();
                            for data in new_prompt.get_data() {
                                match data {
                                    ContextData::Text(text) => final_content.push_str(text.as_str()),
                                    _ => panic!("Not implemented")
                                }
                            }
                            match new_prompt.get_position() {
                                ContextPosition::User => {
                                    messages.push(ChatCompletionMessage { role: MessageRole::user, content: Content::Text(final_content), name:None, tool_calls:None, tool_call_id:None });
                                },
                                ContextPosition::System => {
                                    messages.push(ChatCompletionMessage { role: MessageRole::system, content: Content::Text(final_content), name:None, tool_calls:None, tool_call_id:None });
                                },
                                ContextPosition::AI => {
                                    messages.push(ChatCompletionMessage { role: MessageRole::assistant, content: Content::Text(final_content), name:None, tool_calls:None, tool_call_id:None });
                                },
                                ContextPosition::Total | ContextPosition::Tool => {
                                    messages.push(ChatCompletionMessage { role: MessageRole::tool, content: Content::Text(final_content), name:None, tool_calls:None, tool_call_id:None });
                                }
                            }
                            let messages_clones = messages.clone();
                            let session_clones = session.clone();
                            let model_clone = self.model.clone();
                            let mut client = OpenAIClient::builder()
                                .with_endpoint(self.url.clone())
                                .with_api_key(self.api_key.clone())
                                .build().unwrap();

                            let sender_clone = self.task_sender.clone();
                            let completion = Box::pin( (async move || {

                                let request = ChatCompletionRequest::new(model_clone, messages_clones);
                                sender_clone.send((client.chat_completion(request).await, session_clones)).unwrap()
                            })());
                            self.tasks.write().unwrap().push(completion);
                        }
                    }
                    Ok(())
                }
                else {
                    Err(BackendError::SessionBusy(session))
                }
                
            },
            None => Err(BackendError::SessionMissing(session))
        }
    }
    fn get_whole_current_context_for(&self, session:SessionID) -> Result<WholeContext, BackendError> {
        match self.sessions.get(&session) {
            Some(sess) => Ok(sess.session_data.get_context()),
            None => Err(BackendError::SessionMissing(session))
        }
    }
    fn send_new_prompt(&mut self, new_prompt:WholeContext, session_type:SessionType, config:Option<ChatConfiguration>) -> SessionID {
        let new_session_id = self.latest_session_id;
        self.latest_session_id += 1;
        let session_id = SessionID { id: new_session_id, session_type };
        let mut messages = Vec::with_capacity(new_prompt.len());
        for part in new_prompt.get_parts() {
            // Only supports text for now
            let mut final_content = String::new();
            for data in part.get_data() {
                match data {
                    ContextData::Text(text) => final_content.push_str(text.as_str()),
                    _ => panic!("Not implemented")
                }
            }
            match part.get_position() {
                ContextPosition::User => {
                    messages.push(ChatCompletionMessage { role: MessageRole::user, content: Content::Text(final_content), name:None, tool_calls:None, tool_call_id:None });
                },
                ContextPosition::System => {
                    messages.push(ChatCompletionMessage { role: MessageRole::system, content: Content::Text(final_content), name:None, tool_calls:None, tool_call_id:None });
                },
                ContextPosition::AI => {
                    messages.push(ChatCompletionMessage { role: MessageRole::assistant, content: Content::Text(final_content), name:None, tool_calls:None, tool_call_id:None });
                },
                ContextPosition::Total | ContextPosition::Tool => {
                    messages.push(ChatCompletionMessage { role: MessageRole::tool, content: Content::Text(final_content), name:None, tool_calls:None, tool_call_id:None });
                }
            }
        }
        let messages_clones = messages.clone();
        let session_clones = session_id.clone();
        let model_clone = self.model.clone();
        let mut client = OpenAIClient::builder()
            .with_endpoint(self.url.clone())
            .with_api_key(self.api_key.clone())
            .build().unwrap();
        let sender_clone = self.task_sender.clone();
        let completion = Box::pin( (async move || {sender_clone.send((
            match config {
                Some(config) => {
                    let request = ChatCompletionRequest::new(model_clone, messages_clones)
                        .max_tokens(config.get_max_response() as i64)
                        .temperature(config.get_temp() as f64);
                    client.chat_completion(request).await
                },
                None => {
                    let request = ChatCompletionRequest::new(model_clone, messages_clones);
                    client.chat_completion(request).await
                }
            }
            , session_clones)
        ).unwrap()})());
        {
            self.tasks.write().unwrap().push(completion);
            self.total_tasks += 1;
        }
        self.sessions.insert(session_id, OpenAISession { session_data: OpenAISessionData::ChatComp { messages: messages, context_ver: new_prompt, waiting_on:None }, status: OpenAISessionStatus::Beginning });
        session_id
    }
}

