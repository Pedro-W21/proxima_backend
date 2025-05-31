use std::sync::mpmc::{channel, Receiver, Sender};

use serde::{Deserialize, Serialize};

use crate::database::{chats::SessionType, context::{ContextData, ContextPart, ContextPosition, Prompt, WholeContext}};



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
    RespondToFullPrompt{whole_context:WholeContext, streaming:bool, session_type:SessionType},
    Continue,
}

pub struct EndpointResponse {
    pub variant:EndpointResponseVariant
}
#[derive(Clone, Serialize, Deserialize)]
pub enum EndpointResponseVariant {
    StartStream(ContextData, ContextPosition),
    ContinueStream(ContextData),
    EndStream(ContextData),
    Block(ContextPart)
}