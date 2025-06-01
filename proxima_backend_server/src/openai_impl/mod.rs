use std::{collections::HashMap, future::Future, pin::Pin, sync::{mpmc::{channel, Receiver, Sender}, Arc, RwLock}};

use openai::{chat::{ChatCompletion, ChatCompletionChoice, ChatCompletionGeneric, ChatCompletionMessage, ChatCompletionMessageRole}, ApiResponseOrError, Credentials, OpenAiError};
use proxima_backend::database::context::{ContextData, ContextPart, ContextPosition, Prompt, Response, WholeContext};


use super::{BackendAPI, BackendError, SessionID, SessionType};

#[derive(Clone)]
pub struct OpenAIBackend {
    creds:Credentials,
    model:ChosenModel,
    sessions:HashMap<SessionID, OpenAISession>,
    latest_session_id:usize,
    tasks:Arc<RwLock<Vec<Pin<Box<dyn Future<Output = ()>>>>>>,
    task_sender:Sender<(Result<ChatCompletionGeneric<ChatCompletionChoice>, OpenAiError>, SessionID)>,
    results_recv:Receiver<(Result<ChatCompletionGeneric<ChatCompletionChoice>, OpenAiError>, SessionID)>
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

impl BackendAPI for OpenAIBackend {
    type ConnData = (Credentials, ChosenModel);
    fn new(connection_data:Self::ConnData) -> Self {
        let (send, recv) = channel();
        Self { creds: connection_data.0, model:connection_data.1, sessions:HashMap::with_capacity(16), latest_session_id:0, tasks:Arc::new(RwLock::new(Vec::new())), task_sender:send, results_recv:recv}
    }
    fn send_new_prompt_streaming(&mut self, new_prompt:WholeContext, session_type:SessionType) -> (SessionID, Receiver<ContextData>) {
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
                    messages.push(ChatCompletionMessage { role: ChatCompletionMessageRole::User, content: Some(final_content), ..Default::default() });
                },
                ContextPosition::System => {
                    messages.push(ChatCompletionMessage { role: ChatCompletionMessageRole::System, content: Some(final_content), ..Default::default() });
                },
                ContextPosition::AI => {
                    messages.push(ChatCompletionMessage { role: ChatCompletionMessageRole::Assistant, content: Some(final_content), ..Default::default() });
                },
                ContextPosition::Total => {
                    messages.push(ChatCompletionMessage { role: ChatCompletionMessageRole::Tool, content: Some(final_content), ..Default::default() });
                }
            }
        }
        let messages_clones = messages.clone();
        let session_clones = session_id.clone();
        let model_clone = self.model.clone();
        let creds_clone = self.creds.clone();
        let sender_clone = self.task_sender.clone();
        let (sender_to_client, receiver_for_client) = channel();
        let completion = Box::pin( (async move || {
            let mut receiver = ChatCompletion::builder(model_clone.as_str(), messages_clones).credentials(creds_clone.clone()).create_stream().await.unwrap();
            let mut total = Vec::new();
            loop {
                match receiver.blocking_recv() {
                    Some(completion) => {
                        assert!(completion.choices.len() >= 1);
                        total.push(completion.clone());
                        let choice = &completion.choices[0];
                        match &choice.delta.content {
                            Some(content) => {
                                sender_to_client.send(ContextData::Text(content.clone()));
                            },
                            None => ()
                        }
                    },
                    None => break,
                }
            }    
            sender_clone.send((Ok(total[0].clone().into()), session_clones)).unwrap()
        
        })());
        self.tasks.write().unwrap().push(completion);
        self.sessions.insert(session_id, OpenAISession { session_data: OpenAISessionData::ChatComp { messages: messages, context_ver: new_prompt, waiting_on:None }, status: OpenAISessionStatus::Beginning });
        (session_id, receiver_for_client)
    }
    fn new_empty() -> Self {
        let (send, recv) = channel();
        Self { creds: Credentials::new(String::new(), String::new()), model:String::new(), sessions:HashMap::with_capacity(16), latest_session_id:0, tasks:Arc::new(RwLock::new(Vec::new())), task_sender:send, results_recv:recv}
    
    }
    async fn get_response_to_latest_prompt_for(&mut self, session:SessionID) -> Response {
        let mut task_write = self.tasks.write().unwrap();
        'a: while task_write.len() > 0 {
                let future = task_write.remove(0);
                future.await;
                {
                    while let Ok((result, session_id)) = self.results_recv.recv() {
                        let msg = result.unwrap().choices[0].clone().message;
                        let completion = msg.content.clone();
                        match self.sessions.get_mut(&session_id) {
                            Some(session_data) => {
                                match &mut session_data.session_data {
                                    OpenAISessionData::ChatComp { messages, context_ver, waiting_on } => {
                                        *waiting_on = completion.and_then(|message| {context_ver.add_part(Response::new(vec![ContextData::Text(message.clone())], ContextPosition::AI));Some(Response::new(vec![ContextData::Text(message)], ContextPosition::AI)) });
                                        session_data.status = OpenAISessionStatus::Standby;
                                        messages.push(msg);
                                        if session_id == session {
                                            break 'a;
                                        }
                                    }
                                }
                            }
                            None => ()
                        }
                    }
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
                            messages.push(msg);
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
                            messages.push(msg);
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
                                    messages.push(ChatCompletionMessage { role: ChatCompletionMessageRole::User, content: Some(final_content), ..Default::default() });
                                },
                                ContextPosition::System => {
                                    messages.push(ChatCompletionMessage { role: ChatCompletionMessageRole::System, content: Some(final_content), ..Default::default() });
                                },
                                ContextPosition::AI => {
                                    messages.push(ChatCompletionMessage { role: ChatCompletionMessageRole::Assistant, content: Some(final_content), ..Default::default() });
                                },
                                ContextPosition::Total => {
                                    messages.push(ChatCompletionMessage { role: ChatCompletionMessageRole::Tool, content: Some(final_content), ..Default::default() });
                                }
                            }
    
                            let messages_clones = messages.clone();
                            let session_clones = session.clone();
                            let model_clone = self.model.clone();
                            let creds_clone = self.creds.clone();
                            let sender_clone = self.task_sender.clone();
                            let completion = Box::pin( (async move || {sender_clone.send((ChatCompletion::builder(model_clone.as_str(), messages_clones).credentials(creds_clone).create().await, session_clones)).unwrap()})());
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
    fn send_new_prompt(&mut self, new_prompt:WholeContext, session_type:SessionType) -> SessionID {
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
                    messages.push(ChatCompletionMessage { role: ChatCompletionMessageRole::User, content: Some(final_content), ..Default::default() });
                },
                ContextPosition::System => {
                    messages.push(ChatCompletionMessage { role: ChatCompletionMessageRole::System, content: Some(final_content), ..Default::default() });
                },
                ContextPosition::AI => {
                    messages.push(ChatCompletionMessage { role: ChatCompletionMessageRole::Assistant, content: Some(final_content), ..Default::default() });
                },
                ContextPosition::Total => {
                    messages.push(ChatCompletionMessage { role: ChatCompletionMessageRole::Tool, content: Some(final_content), ..Default::default() });
                }
            }
        }
        let messages_clones = messages.clone();
        let session_clones = session_id.clone();
        let model_clone = self.model.clone();
        let creds_clone = self.creds.clone();
        let sender_clone = self.task_sender.clone();
        let completion = Box::pin( (async move || {sender_clone.send((ChatCompletion::builder(model_clone.as_str(), messages_clones).credentials(creds_clone).create().await, session_clones)).unwrap()})());
        self.tasks.write().unwrap().push(completion);
        self.sessions.insert(session_id, OpenAISession { session_data: OpenAISessionData::ChatComp { messages: messages, context_ver: new_prompt, waiting_on:None }, status: OpenAISessionStatus::Beginning });
        session_id
    }
}

