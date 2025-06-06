use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{access_modes::AccessModeID, context::{ContextPart, WholeContext}, devices::DeviceID, tags::TagID};

pub type ChatID = usize;


#[derive(Clone, Copy, Hash, PartialEq, Eq,Serialize, Deserialize, Debug)]
pub struct SessionID {
    pub id:usize,
    pub session_type:SessionType
}

#[derive(Clone, Copy, Hash, PartialEq, Eq,Serialize, Deserialize, Debug)]
pub enum SessionType {
    Function,
    Chat,
    Completion
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Chat {
    pub id:ChatID,
    pub context:WholeContext,
    pub chat_title:Option<String>,
    pub session_id:Option<SessionID>,
    pub origin_device:DeviceID, 
    pub start_date:DateTime<Utc>,
    pub latest_message:DateTime<Utc>,
    pub tags:Vec<TagID>,
    pub access_modes:Vec<AccessModeID>
}

impl Chat {
    pub fn get_context(&self) -> &WholeContext {
        &self.context
    }
    pub fn get_title(&self) -> &Option<String> {
        &self.chat_title
    }
    pub fn get_session_id(&self) -> &Option<SessionID> {
        &self.session_id
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Chats {
    all_chats:HashMap<ChatID, Chat>,
}

impl Chats {
    pub fn new() -> Self {
        Self { all_chats:HashMap::with_capacity(4096) }
    }
    pub fn add_context_part_to(&mut self, context_part:ContextPart, chat_id:ChatID) {
        match self.all_chats.get_mut(&chat_id) {
            Some(chat) => chat.context.add_part(context_part),
            None => ()
        }
    }
    pub fn get_chats(&self) -> &HashMap<ChatID, Chat> {
        &self.all_chats
    }
    pub fn get_chats_mut(&mut self) -> &mut HashMap<ChatID, Chat> {
        &mut self.all_chats
    }
    pub fn create_chat(&mut self, starting_context:WholeContext, session_id:Option<SessionID>, origin_device:DeviceID) -> ChatID {
        let id = self.all_chats.len();
        self.all_chats.insert(id, Chat {
            context: starting_context,
            chat_title: None,
            session_id,
            origin_device,
            id,
            tags:Vec::new(),
            access_modes:vec![0],
            latest_message:Utc::now(),
            start_date:Utc::now()
        });
        id
    }
    pub fn update_chat(&mut self, chat:Chat) {
        let id = chat.id;
        self.all_chats.insert(id, chat);
    }
    pub fn add_chat_raw(&mut self, mut chat:Chat) {
        let id = self.all_chats.len();
        chat.id = id;
        self.all_chats.insert(id, chat);
    }
}