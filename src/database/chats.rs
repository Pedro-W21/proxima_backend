use std::collections::{HashMap, HashSet};

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::database::{context::ContextPosition, configuration::{ChatConfiguration, ChatConfigID}};

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
    pub waiting_on_response:bool,
    pub latest_message:DateTime<Utc>,
    pub tags:HashSet<TagID>,
    pub access_modes:HashSet<AccessModeID>,
    pub config:Option<ChatConfigID>
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
    pub fn get_id(&self) -> ChatID {
        self.id
    }
    pub fn add_to_context(&mut self, new_context:ContextPart) {
        let waiting_on_response = 
        match new_context.get_position() {
            &ContextPosition::User | &ContextPosition::System => false,
            _ => true
        };
        self.waiting_on_response = waiting_on_response;
        self.context.add_part(new_context);
        self.latest_message = Utc::now();
    }
    pub fn is_waiting_on_response(&self) -> bool {
        self.waiting_on_response
    }

    pub fn last_response_is_user(&self) -> bool {
        match self.context.get_parts().last().unwrap().get_position() {
            &ContextPosition::User => true,
            _ => false
        }
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
    pub fn create_chat(&mut self, starting_context:WholeContext, session_id:Option<SessionID>, origin_device:DeviceID, config:Option<ChatConfigID>) -> ChatID {
        let id = self.all_chats.len();
        self.all_chats.insert(id, Chat {
            context: starting_context,
            chat_title: None,
            session_id,
            origin_device,
            id,
            tags:HashSet::new(),
            access_modes:HashSet::from([0]),
            latest_message:Utc::now(),
            start_date:Utc::now(),
            waiting_on_response:true,
            config
        });
        id
    }
    pub fn update_chat(&mut self, chat:Chat) {
        let id = chat.id;
        self.all_chats.insert(id, chat);
    }
    pub fn add_chat_raw(&mut self, mut chat:Chat) -> ChatID {
        let id = self.all_chats.len();
        chat.id = id;
        self.all_chats.insert(id, chat);
        id
    }
    pub fn get_last_chat(&self) -> Option<&Chat> {
        if self.all_chats.len() > 0 {
            self.all_chats.get(&(self.all_chats.len() - 1))
        }
        else {
            None
        }
    }
}