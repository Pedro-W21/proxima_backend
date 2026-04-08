use std::{net::SocketAddr, sync::mpmc::Receiver};

use crate::database::{DatabaseSender, chats::{SessionID, SessionType}, configuration::ChatConfiguration, context::{ContextData, ContextPart, Prompt, Response, WholeContext}};
use serde::{Deserialize, Serialize};






pub enum BackendError {
    SessionMissing(SessionID),
    SessionBusy(SessionID),
    BackendUnavailable
}

pub trait BackendAPI {
    type ConnData:Send + Sync + Clone;
    fn new(connection_data:Self::ConnData) -> Self;
    fn new_empty() -> Self;
    fn send_new_prompt(&mut self, new_prompt:WholeContext, session_type:SessionType, config:Option<ChatConfiguration>, db_sender:DatabaseSender) -> Result<SessionID, BackendError>;
    fn send_new_prompt_streaming(&mut self, new_prompt:WholeContext, session_type:SessionType, config:Option<ChatConfiguration>, db_sender:DatabaseSender) -> Result<(SessionID, Receiver<ContextData>), BackendError>;
    fn try_get_response_to_latest_prompt_for(&mut self, session:SessionID) -> Option<Response>;
    fn get_response_to_latest_prompt_for_blocking(&mut self, session:SessionID) -> Response;
    fn get_response_to_latest_prompt_for(&mut self, session:SessionID) -> impl std::future::Future<Output = Result<Response, BackendError>> + Send;
}