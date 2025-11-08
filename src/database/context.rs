use serde::{Deserialize, Serialize};

use crate::database::configuration::{ChatConfiguration, ChatSetting, RepeatPosition};

pub type Prompt = ContextPart;
pub type Response = ContextPart;
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct ContextPart {
    data:Vec<ContextData>,
    position:ContextPosition
}

impl ContextPart {
    pub fn new(data:Vec<ContextData>, position:ContextPosition) -> ContextPart {
        ContextPart { data, position }
    }
    pub fn get_data(&self) -> &Vec<ContextData> {
        &self.data
    }
    pub fn get_data_mut(&mut self) -> &mut Vec<ContextData> {
        &mut self.data
    }
    pub fn merge_data_with(&mut self, append:ContextPart) {
        for data in append.data {
            self.add_data(data);
        }
    }
    pub fn add_data(&mut self, data:ContextData) {
        self.data.push(data);
    }
    pub fn new_user_prompt_with_tools(mut data:Vec<ContextData>) -> ContextPart {
        data.insert(0, ContextData::Text("<user_prompt>\n".to_string()));
        data.push(ContextData::Text("</user_prompt>\n".to_string()));
        Self { data: data, position: ContextPosition::User }
    }
    pub fn get_position(&self) -> &ContextPosition {
        &self.position
    }
    pub fn in_visible_position(&self) -> bool {
        match self.position {
            ContextPosition::System => false,
            _ => true
        }
    }
    pub fn is_user(&self) -> bool {
        match self.position {
            ContextPosition::User => true,
            _ => false
        }
    }
    pub fn data_to_text(&self) -> Vec<String> {
        self.data.iter().map(|part| {
            match part {
                ContextData::Text(text) => text.clone(),
                ContextData::Image(img) => String::from("Alt text : This is an image. Not much more I can say since alt text is not implemented yet")
            }
        }).collect()
    }
    pub fn concatenate_text(&mut self) {
        let mut new_data = Vec::with_capacity(self.data.len());
        let mut current_string = String::new();
        while self.data.len() > 0 {
            let first_elem = self.data.remove(0);
            match first_elem {
                ContextData::Text(text) => current_string += &text,
                ContextData::Image(image) => {
                    new_data.push(ContextData::Text(current_string));
                    current_string = String::new();
                },
            }
        }
        if !current_string.is_empty() {
            new_data.push(ContextData::Text(current_string));
        }
        self.data = new_data;
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum ContextPosition {
    System,
    User,
    AI,
    Total,
    Tool
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContextData {
    Text(String),
    Image(usize),
}

impl ContextData {
    pub fn get_text(&self) -> String {
        match self {
            Self::Text(txt) => txt.clone(),
            _ => panic!("Not text when it should be !")
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WholeContext {
    parts:Vec<ContextPart>
}

impl WholeContext {
    pub fn new_with_all_settings(mut parts:Vec<ContextPart>, settings:&ChatConfiguration) -> Self {
        parts.insert(0, settings.get_full_system_prompt());
        let mut pre_self = Self {parts};
        pre_self.add_per_turn_settings(settings);
        pre_self
    }
    pub fn new(parts:Vec<ContextPart>) -> Self {
        Self { parts }
    }
    pub fn get_parts_mut(&mut self) -> &mut Vec<ContextPart> {
        &mut self.parts
    }
    pub fn get_whole_system_prompt(&self) -> WholeContext {
        let system = self.parts.iter().filter_map(|part| { match part.get_position() {ContextPosition::System => Some(part.clone()), _ => None} }).collect::<Vec<ContextPart>>();
        WholeContext { parts: system }
    }
    pub fn get_everything_but_system_prompt(&self) -> WholeContext {
        let system = self.parts.iter().filter_map(|part| { match part.get_position() {ContextPosition::System => None, _ => Some(part.clone())} }).collect::<Vec<ContextPart>>();
        WholeContext { parts: system }
    }
    pub fn add_per_turn_settings(&mut self, settings:&ChatConfiguration) {
        for setting in settings.get_raw_settings() {
            match setting {
                ChatSetting::RepeatedPrePrompt(prompt, position) => match position {
                    RepeatPosition::AfterLatest => self.parts.push(prompt.clone()),
                    RepeatPosition::BeforeLatest => if self.parts.len() >= 1 {
                        self.parts.insert(self.parts.len() - 1,prompt.clone())
                    }
                    else {
                        self.parts.insert(0,prompt.clone())
                    }
                },
                _ => ()
            }
        }
        match settings.get_tools() {
            Some(tools) => {
                self.parts.push(tools.get_tool_data_insert());
            },
            None => ()
        }
        
    }
    pub fn merge_with(mut self, mut other:WholeContext) -> WholeContext {
        self.parts.append(&mut other.parts);
        Self { parts: self.parts }
    }
    pub fn concatenate_into_single_part(&self) -> ContextPart {
        let mut data = Vec::with_capacity(self.parts.len() * 10);
        for part in &self.parts {
            data.extend(part.data.iter().cloned());
        }
        ContextPart { data, position:ContextPosition::Total }
    }
    pub fn add_part(&mut self, part:ContextPart) {
        self.parts.push(part);
    }
    pub fn get_parts(&self) -> &Vec<ContextPart> {
        &self.parts
    }
    pub fn len(&self) -> usize {
        self.parts.len()
    }
}