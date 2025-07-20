use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{ai_interaction::tools::{ProximaTool, Tools}, database::{access_modes::AccessModeID, context::{ContextPart, ContextPosition, WholeContext}, tags::TagID}};


pub type ChatConfigID = usize;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatConfiguration {
    pub id: ChatConfigID,
    pub created_on:DateTime<Utc>,
    pub last_updated:DateTime<Utc>,
    raw_settings:Vec<ChatSetting>,
    tools:Option<Tools>,
    pub tags:HashSet<TagID>,
    pub access_modes:HashSet<AccessModeID>,
}   

impl ChatConfiguration {
    pub fn get_raw_settings(&self) -> &Vec<ChatSetting> {
        &self.raw_settings
    }
    pub fn get_tools(&self) -> &Option<Tools> {
        &self.tools
    }
    pub fn set_tools(&mut self, new_tools:Option<Tools>) {
        self.tools = new_tools;
    }
    pub fn get_full_system_prompt(&self) -> ContextPart {
        let mut system_prompt = ContextPart::new(vec![], ContextPosition::System);
        for setting in &self.raw_settings {
            match setting {
                ChatSetting::SystemPrompt(prompt) => system_prompt.merge_data_with(prompt.clone()),
                _ => ()
            }
        }
        match &self.tools {
            Some(tools) => system_prompt.merge_data_with(tools.get_tool_calling_sys_prompt()),
            None => ()
        }
        system_prompt
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChatSetting {
    SystemPrompt(ContextPart),
    Temperature(u64),
    ResponseTokenLimit(usize),
    MaxContextLength(usize),
    AccessMode(AccessModeID),
    PrePrompt(ContextPart),
    PrePromptBeforeLatest(ContextPart),
    Tool(ProximaTool)
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatConfigurations {
    pub all_configs:Vec<ChatConfiguration>
}

impl ChatConfigurations {
    pub fn new() -> Self {
        Self { all_configs: Vec::with_capacity(1000) }
    }
    pub fn add_config(&mut self, mut config:ChatConfiguration) -> ChatConfigID {
        let id = self.all_configs.len();
        config.id = id;
        self.all_configs.push(config);
        id
    }
    pub fn update_config(&mut self, config:ChatConfiguration) {
        let id = config.id;
        self.all_configs[id] = config;
    }
    pub fn insert_config(&mut self, config:ChatConfiguration) {
        let id = config.id;
        for config in &mut self.all_configs[id..] {
            config.id += 1
        }
        self.all_configs.insert(id,config);
    }
    pub fn get_configs(&self) -> &Vec<ChatConfiguration> {
        &self.all_configs
    }
    pub fn get_configs_mut(&mut self) -> &mut Vec<ChatConfiguration> {
        &mut self.all_configs
    }
}