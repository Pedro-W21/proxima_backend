use std::{collections::{HashMap, HashSet}, fs::File, io::{Read, Write}, path::PathBuf};

use chrono::{DateTime, Utc};
use rand::{Rng, rng};
use serde::{Deserialize, Serialize};

use crate::database::{access_modes::AccessModeID, tags::TagID};

pub type MemoryID = usize;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Memory {
    pub add_date:DateTime<Utc>,
    pub last_update:DateTime<Utc>,
    pub access_modes:HashSet<AccessModeID>,
    pub tags:HashSet<TagID>,
    file_name:String,
    pub id:MemoryID,
}

impl Memory {
    pub fn new(access_modes:HashSet<AccessModeID>, tags:HashSet<TagID>) -> Self {
        Self { add_date: Utc::now(), last_update: Utc::now(), access_modes, tags, file_name: String::new(), id: 0 }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Memories {
    pub memories:HashMap<MemoryID, Memory>,
    pub last_memory_id:MemoryID,
}

impl Memories {
    pub fn new() -> Self {
        Self { memories: HashMap::with_capacity(256), last_memory_id: 0 }
    }
    pub fn add_memory(&mut self, data:String, mut access_modes:HashSet<AccessModeID>, tags:HashSet<TagID>, proxima_data_path:PathBuf) -> MemoryID {
        let id = self.last_memory_id;
        self.last_memory_id += 1;
        access_modes.insert(0);
        let mut found_path = false;
        let mut test_path = proxima_data_path.clone();
        let mut file_name = format!("{}{id}.txt", access_modes.iter().map(|mode| {format!("{mode}_")}).collect::<Vec<String>>().concat());
        while !found_path {
            test_path.push(format!("memories/{}", file_name.clone()));
            if test_path.exists() {
                test_path = proxima_data_path.clone();
                file_name = format!("{}.txt", id + rng().random_range(0..100));
            }
            else {
                found_path = true;
            }
        }
        match File::create(test_path) {
            Ok(mut file) => file.write_all(data.as_bytes()).expect("File should be writable"),
            Err(e) => panic!("File should be creatable by now, error : {e}")
        }
        let time = Utc::now();
        let memory = Memory { 
            tags,
            access_modes,
            add_date:time,
            last_update:time,
            id,
            file_name
        };
        self.memories.insert(id, memory);
        id
    }
    pub fn update_memory(&mut self, id:MemoryID, data:String, proxima_data_path:PathBuf) {
        self.memories.get_mut(&id).and_then(|memory| {
            match File::create(proxima_data_path.join(format!("memories/{}", memory.file_name))) {
                Ok(mut file) => file.write_all(data.as_bytes()).expect("File should be writable"),
                Err(e) => panic!("File should be creatable by now, error : {e}")
            };
            memory.last_update = Utc::now();
            Option::<u8>::None
        });
    }
    pub fn retrieve_ids(&self, request:MemoryRequest) -> Vec<MemoryID> {
        let mut retrieved = Vec::with_capacity(4);
        for (id, memory) in &self.memories {
            if memory.last_update >= request.from && memory.last_update <= request.to && memory.access_modes.intersection(&request.access_modes).count() > 0 {
                match &request.tags {
                    Some(tags) => if memory.tags.intersection(tags).count() > 0 {
                        retrieved.push(*id);
                    },
                    None => retrieved.push(*id),
                }
            }
        }
        retrieved
    }
    pub fn retrieve_data_from_ids(&self, ids:Vec<MemoryID>, proxima_data_path:PathBuf) -> Vec<(Memory, String)> {
        let mut retrieved = Vec::with_capacity(ids.len());
        for id in ids {
            self.memories.get(&id).map(|memory| {
                match File::open(proxima_data_path.join(format!("memories/{}", memory.file_name.clone()))) {
                    Ok(mut file) => {
                        let mut string = String::with_capacity(512);
                        file.read_to_string(&mut string).expect("Memory could not be read back");
                        retrieved.push((memory.clone(), string));
                    },
                    Err(e) => panic!("File should exist")
                }
            });
        }
        retrieved
    }
    pub fn get_memory_with_data(&self, memory_id:MemoryID, proxima_data_path:PathBuf) -> Option<(&Memory, String)> {
        match self.memories.get(&memory_id) {
            Some(memory) => match File::open(proxima_data_path.join(PathBuf::from(format!("memories/{}", memory.file_name)))) {
                Ok(mut file) => {
                    let mut string = String::with_capacity(512);
                    file.read_to_string(&mut string).unwrap();
                    Some((memory, string))
                },
                Err(_) => None
            },
            None => None
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MemoryRequest {
    from:DateTime<Utc>,
    to:DateTime<Utc>,
    tags:Option<HashSet<TagID>>,
    access_modes:HashSet<AccessModeID>
}


impl MemoryRequest {
    pub fn new(from:DateTime<Utc>, to:DateTime<Utc>, access_modes:HashSet<AccessModeID>, tags:Option<HashSet<TagID>>) -> Self {
        Self { from, to, tags, access_modes }
    }
}
