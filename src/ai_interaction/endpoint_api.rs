use std::sync::mpmc::{channel, sync_channel, Receiver, Sender};

use serde::{Deserialize, Serialize};

use crate::database::{access_modes::AccessModeID, chats::{ChatID, SessionType}, configuration::ChatConfiguration, context::{ContextData, ContextPart, ContextPosition, Prompt, WholeContext}};



pub struct EndpointRequest {
    pub variant:EndpointRequestVariant,
    pub response_tunnel:Sender<EndpointResponse>
}

impl EndpointRequest {
    pub fn new(variant:EndpointRequestVariant) -> (Self, Receiver<EndpointResponse>) {
        let (response_tunnel, receiver_tunnel) = channel();
        (
            Self {
                variant,
                response_tunnel
            },
            receiver_tunnel
        )
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum EndpointRequestVariant {
    RespondToFullPrompt{whole_context:WholeContext, streaming:bool, session_type:SessionType, chat_settings:Option<ChatConfiguration>, chat_id:Option<ChatID>},
    Continue,
}

impl EndpointRequestVariant {
    pub fn is_stream(&self) -> bool {
        match self {
            EndpointRequestVariant::RespondToFullPrompt { whole_context, streaming, session_type, chat_settings,chat_id } => *streaming,
            EndpointRequestVariant::Continue => false
        }
    }
}


pub struct EndpointResponse {
    pub variant:EndpointResponseVariant
}
#[derive(Clone, Serialize, Deserialize)]
pub enum EndpointResponseVariant {
    StartStream(ContextData, ContextPosition),
    ContinueStream(ContextData, ContextPosition),
    EndStream(ContextData, ContextPosition),
    Block(ContextPart),
    MultiTurnBlock(WholeContext),
    EndpointError(EndpointError)
}

#[derive(Clone, Serialize, Deserialize)]
pub enum EndpointError {
    BackendUnavailable{url:String}
}