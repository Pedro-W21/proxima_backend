use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::database::memories::MemoryID;

use super::tags::TagID;

pub type AccessModeID = usize;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum AMSetting {
    Bool(bool),
    Integer(i64),
    String(String),
    Float(f64)
}

impl Eq for AMSetting {

}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessMode {
    id:AccessModeID,
    pub tags:HashSet<TagID>,
    pub added_on:DateTime<Utc>,
    pub name:String,
    pub persistent_memory:Option<MemoryID>,
    pub am_settings:HashMap<String, AMSetting>
}

impl AccessMode {
    pub fn new(id:AccessModeID, tags:HashSet<TagID>, name:String) -> Self {
        Self { id, tags, name, added_on:Utc::now(), persistent_memory:None, am_settings:HashMap::with_capacity(8) }
    }
    pub fn with_settings(mut self, settings:HashMap<String, AMSetting>) -> Self {
        self.am_settings = settings;
        self
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
    all_modes:HashMap<AccessModeID, AccessMode>,
    pub latest_id:AccessModeID,
}

impl AccessModes {
    pub fn new() -> Self {
        Self { 
            all_modes: HashMap::from([
                (0, AccessMode {added_on:Utc::now(),id:0, tags:HashSet::new(), name:String::from("global"), persistent_memory:None, am_settings:HashMap::new()}),
                (1, AccessMode {added_on:Utc::now(),id:1, tags:HashSet::new(), name:String::from("callbacks"), persistent_memory:None, am_settings:HashMap::new()}),
            ]),
            latest_id:2
        }
    }
    pub fn get_modes(&self) -> &HashMap<AccessModeID, AccessMode> {
        &self.all_modes
    }
    pub fn get_modes_mut(&mut self) -> &mut HashMap<AccessModeID, AccessMode> {
        &mut self.all_modes
    }
    pub fn update_mode(&mut self, mode:AccessMode) -> bool {
        let num = mode.id;
        self.all_modes.insert(num, mode).is_some()
    }
    pub fn associate_tag_to_mode(&mut self, mode:AccessModeID, tag:TagID) -> bool {
        self.all_modes.get_mut(&0).unwrap().tags.insert(tag);
        self.all_modes.get_mut(&mode).map(|am| {am.tags.insert(tag)}).unwrap_or(false)
    }
    pub fn get_updated_modes_from_association(&self, mode:AccessModeID, tag:TagID) -> Option<(AccessMode, AccessMode)> {
        let mut mode_0 = self.all_modes.get(&0).unwrap().clone();
        mode_0.tags.insert(tag);
        self.all_modes.get(&mode).map(|am| {
            let mut clone = am.clone();
            clone.tags.insert(tag);
            (mode_0, clone)
        })
    }
    pub fn add_mode(&mut self, mut mode:AccessMode) -> AccessModeID {
        let num = self.latest_id;
        self.latest_id += 1;
        mode.id = num;
        self.all_modes.insert(num, mode);
        num
    }
}