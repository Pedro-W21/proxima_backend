use std::{collections::{HashMap, HashSet}, fs::File, io::{Read, Write}, path::PathBuf};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};

use crate::database::{access_modes::AccessModeID, tags::TagID};


#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaStorage {
    pub data:HashMap<MediaHash, Media>
}

impl MediaStorage {
    pub fn new() -> Self {
        Self { data: HashMap::with_capacity(512) }
    }
    pub fn add_media(&mut self, data:Vec<u8>, tags:HashSet<TagID>, mut access_modes:HashSet<AccessModeID>, original_file_name:String, proxima_data_path:PathBuf, media_type:MediaType) -> MediaHash {
        
        let mut hasher = Sha3_256::new();
        hasher.update(&data);
        let hash:[u8 ; 32] = hasher.finalize().into();
        access_modes.insert(0);
        let mut found_path = false;
        let mut test_path = proxima_data_path.clone();
        let mut file_name = original_file_name.clone();
        let mut bytes_added = 0;
        while !found_path {
            test_path.push(format!("media/{}", file_name.clone()));
            if test_path.exists() {
                test_path = proxima_data_path.clone();
                file_name = format!("{:#04x}", hash[bytes_added]) + &file_name;
                bytes_added += 1;
            }
            else {
                found_path = true;
            }
        }
        match File::create(test_path) {
            Ok(mut file) => file.write_all(&data).expect("File should be writable"),
            Err(e) => panic!("File should be creatable by now, error : {e}")
        }
        let time = Utc::now();
        let media = Media { 
            hash,
            media_type,
            file_name, 
            tags,
            access_modes,
            added_at: time 
        };
        self.data.insert(hash, media);
        hash
    }   
    pub fn get_media(&self, hash:&MediaHash) -> Option<&Media> {
        self.data.get(hash)
    }
    pub fn get_media_with_data(&self, hash:&MediaHash, proxima_data_path:PathBuf) -> Option<(&Media, Vec<u8>)> {
        match self.data.get(hash) {
            Some(media) => match File::open(proxima_data_path.join(PathBuf::from(format!("media/{}", media.file_name)))) {
                Ok(mut file) => {
                    let mut data = Vec::with_capacity(100_000);
                    file.read_to_end(&mut data).unwrap();
                    Some((media, data))
                },
                Err(_) => None
            },
            None => None
        }
    }
    pub fn update_media(&mut self, mut new_media:Media, new_data:Vec<u8>, proxima_data_path:PathBuf) {
        self.data.remove(&new_media.hash);
        self.add_media(new_data, new_media.tags, new_media.access_modes, new_media.file_name, proxima_data_path, new_media.media_type);
    }
    pub fn insert_media_raw(&mut self, media:Media) {
        self.data.insert(media.hash, media);
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Media {
    pub hash:MediaHash,
    pub media_type:MediaType,
    pub file_name:String,
    pub tags:HashSet<TagID>,
    pub access_modes:HashSet<AccessModeID>,
    pub added_at:DateTime<Utc>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MediaType {
    Image,
    Video,
    Audio
}

pub type MediaHash = [u8 ; 32];