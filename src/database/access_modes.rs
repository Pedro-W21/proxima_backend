use serde::{Deserialize, Serialize};

use super::tags::TagID;

pub type AccessModeID = usize;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessMode {
    id:AccessModeID,
    tags:Vec<TagID>,
    name:String
}

impl AccessMode {
    pub fn get_name(&self) -> &String {
        &self.name
    }
    pub fn get_tags(&self) -> &Vec<TagID> {
        &self.tags
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessModes {
    all_modes:Vec<AccessMode>
}

impl AccessModes {
    pub fn new() -> Self {
        Self { all_modes: vec![AccessMode {id:0, tags:Vec::new(), name:String::from("global")}] }
    }
    pub fn get_modes(&self) -> &Vec<AccessMode> {
        &self.all_modes
    }
    pub fn get_modes_mut(&mut self) -> &mut Vec<AccessMode> {
        &mut self.all_modes
    }
    pub fn update_mode(&mut self, mode:AccessMode) {
        let num = mode.id;
        self.all_modes[num] = mode;
    }
    pub fn add_mode(&mut self, mut mode:AccessMode) -> AccessModeID {
        let num = self.all_modes.len();
        mode.id = num;
        self.all_modes.push(mode);
        num
    }
}