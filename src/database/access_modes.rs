use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::tags::TagID;

pub type AccessModeID = usize;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessMode {
    id:AccessModeID,
    pub tags:HashSet<TagID>,
    pub added_on:DateTime<Utc>,
    name:String
}

impl AccessMode {
    pub fn new(id:AccessModeID, tags:HashSet<TagID>, name:String) -> Self {
        Self { id, tags, name, added_on:Utc::now() }
    }
    pub fn get_name(&self) -> &String {
        &self.name
    }
    pub fn set_id(&mut self, id:AccessModeID) {
        self.id = id;
    }
    pub fn get_tags(&self) -> &HashSet<TagID> {
        &self.tags
    }
    pub fn get_id(&self) -> AccessModeID {
        self.id
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessModes {
    all_modes:Vec<AccessMode>
}

impl AccessModes {
    pub fn new() -> Self {
        Self { all_modes: vec![AccessMode {added_on:Utc::now(),id:0, tags:HashSet::new(), name:String::from("global")}] }
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
    pub fn associate_tag_to_mode(&mut self, mode:AccessModeID, tag:TagID) -> bool {
        self.all_modes[0].tags.insert(tag);
        self.all_modes[mode].tags.insert(tag)
    }
    pub fn add_mode(&mut self, mut mode:AccessMode) -> AccessModeID {
        let num = self.all_modes.len();
        mode.id = num;
        self.all_modes.push(mode);
        num
    }
}