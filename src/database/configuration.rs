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
    pub raw_settings:Vec<ChatSetting>,
    pub tools:Option<Tools>,
    pub tags:HashSet<TagID>,
    pub access_modes:HashSet<AccessModeID>,
    pub name:String,
}   

impl ChatConfiguration {
    pub fn new(name:String, raw_settings:Vec<ChatSetting>) -> Self {
        Self { id: 0, created_on: Utc::now(), last_updated: Utc::now(), raw_settings:raw_settings.clone(), tools: Tools::try_from_settings(raw_settings), tags: HashSet::new(), access_modes: HashSet::from([0]), name }
    }
    pub fn new_with_tags_access_modes(name:String, raw_settings:Vec<ChatSetting>, tags:HashSet<TagID>, access_modes:HashSet<AccessModeID>) -> Self {
        Self { id: 0, created_on: Utc::now(), last_updated: Utc::now(), raw_settings:raw_settings.clone(), tools: Tools::try_from_settings(raw_settings), tags, access_modes, name }
    
    }
    pub fn get_raw_settings(&self) -> &Vec<ChatSetting> {
        &self.raw_settings
    }
    pub fn get_temp(&self) -> f64 {
        match self.raw_settings.iter().find(|setting| {match setting {ChatSetting::Temperature(temp) => true, _ => false}}) {
            Some(setting) => match setting {ChatSetting::Temperature(temp) => (*temp as f64/100.0), _ => panic!("Should be temp, impossible that it isn't")},
            None => 0.7
        }
    }
    pub fn get_max_context(&self) -> usize {
        match self.raw_settings.iter().find(|setting| {match setting {ChatSetting::MaxContextLength(ctx) => true, _ => false}}) {
            Some(setting) => match setting {ChatSetting::MaxContextLength(ctx) => *ctx, _ => panic!("Should be temp, impossible that it isn't")},
            None => 16184
        }
    }
    pub fn get_max_response(&self) -> usize {
        match self.raw_settings.iter().find(|setting| {match setting {ChatSetting::ResponseTokenLimit(ctx) => true, _ => false}}) {
            Some(setting) => match setting {ChatSetting::ResponseTokenLimit(ctx) => *ctx, _ => panic!("Should be temp, impossible that it isn't")},
            None => 2048
        }
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

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum ChatSetting {
    SystemPrompt(ContextPart),
    Temperature(u64),
    ResponseTokenLimit(usize),
    MaxContextLength(usize),
    AccessMode(AccessModeID),
    PrePrompt(ContextPart),
    RepeatedPrePrompt(ContextPart, RepeatPosition),
    Tool(ProximaTool)
}


#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum RepeatPosition {
    BeforeLatest,
    AfterLatest,
}

impl ChatSetting {
    pub fn get_title(&self) -> String {
        match self {
            Self::Tool(tool) => format!("Tool : {}", tool.get_name()),
            Self::AccessMode(access_mode) => format!("Access mode"),
            Self::MaxContextLength(max_ctx) => format!("Max context : {}", *max_ctx),
            Self::ResponseTokenLimit(limit) => format!("Response limit : {}", *limit),
            Self::PrePrompt(pre_prompt) => format!("Pre-prompt : initial"),
            Self::RepeatedPrePrompt(pre_prompt, _) => format!("repeated pre-prompt"),
            Self::Temperature(temp) => format!("Temperature : {}", *temp as f64/100.0),
            Self::SystemPrompt(system_prompt) => format!("System prompt"),
        }
    }
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