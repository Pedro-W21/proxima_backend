use serde::{Deserialize, Serialize};

use crate::{ai_interaction::tools::{ProximaTool, Tools}, database::{access_modes::AccessModeID, context::{ContextPart, ContextPosition, WholeContext}}};


#[derive(Clone, Serialize, Deserialize)]
pub struct ChatSettings {
    raw_settings:Vec<ChatSetting>,
    tools:Option<Tools>
}   

impl ChatSettings {
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

#[derive(Clone, Serialize, Deserialize)]
pub enum ChatSetting {
    SystemPrompt(ContextPart),
    Temperature(f64),
    ResponseTokenLimit(usize),
    MaxContextLength(usize),
    AccessMode(AccessModeID),
    PrePrompt(ContextPart),
    PrePromptBeforeLatest(ContextPart),
    Tool(ProximaTool)
}