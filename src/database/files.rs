use std::{collections::HashSet, ffi::OsString, fs::{self, File}, io::Read};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{access_modes::AccessModeID, description::Description, devices::DeviceID, folders::{AbsolutePath, Folders, ProxFolder}, tags::TagID};



pub type FileID = usize;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProxFile {
    id:FileID,
    absolute_path:AbsolutePath,
    tags:HashSet<TagID>,
    desc:Option<Description>,
    name:String,
    extension:Option<String>,
    from_device:DeviceID,
    added_at:DateTime<Utc>,
    last_modified:DateTime<Utc>,
    access_modes:HashSet<AccessModeID>,
}

impl ProxFile {
    pub fn get_id(&self) -> FileID {
        self.id
    }
    pub fn is_pure_utf8(&self) -> bool {
        match File::open(self.absolute_path.clone()) {
            Ok(mut open_file) => {
                let mut string = String::with_capacity(1024);
                match open_file.read_to_string(&mut string) {
                    Ok(bytes_read) => true,
                    Err(error) => false
                }
            },
            Err(error) => panic!("File that's supposed to exist doesn't {}", self.absolute_path.clone().to_string_lossy().to_string()),
        }
    }
    pub fn add_desc_tags(&mut self, desc:Description, tags:HashSet<TagID>) {
        self.desc = Some(desc);
        self.tags = tags
    }
    pub fn get_path(&self) -> AbsolutePath {
        self.absolute_path.clone()
    }
    pub fn get_pure_utf8(&self) -> String {
        match File::open(self.absolute_path.clone()) {
            Ok(mut open_file) => {
                let mut string = String::with_capacity(1024);
                match open_file.read_to_string(&mut string) {
                    Ok(bytes_read) => string,
                    Err(error) => panic!("File is supposed to be pure UTF-8")
                }
            },
            Err(error) => panic!("File that's supposed to exist doesn't {}", self.absolute_path.clone().to_string_lossy().to_string()),
        }
    }
    pub fn get_desc(&self) -> Option<Description> {
        self.desc.clone()
    }
    pub fn get_name_string_lossy(&self) -> String {
        self.name.clone()
    }
    pub fn get_extension_lossy(&self) -> Option<String> {
        self.extension.as_ref().and_then(|ext| {Some(ext.clone())})
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Files {
    all_files:Vec<ProxFile>,
    number_of_files:usize
}

pub struct NewFile {
    absolute_path:AbsolutePath,
    from_device:DeviceID
}

impl NewFile {
    pub fn new(absolute_path:AbsolutePath, from_device:DeviceID) -> Self {
        Self { absolute_path, from_device }
    }
}

impl Files {
    pub fn new() -> Self {
        Self { all_files: Vec::with_capacity(2000), number_of_files: 0 }
    }
    pub fn len(&self) -> usize {
        self.all_files.len()
    }
    pub fn get_file_mut(&mut self, id:FileID) -> &mut ProxFile {
        &mut self.all_files[id]
    }
    pub fn get_file_by_id(&self, id:FileID) -> &ProxFile {
        &self.all_files[id]
    }
    pub fn take_next_file(&mut self) -> FileID {
        let id = self.number_of_files;
        self.number_of_files += 1;
        id
    }
    pub fn file_exists(&self, path:AbsolutePath) -> bool {
        self.all_files.iter().find(|file| {&file.absolute_path == &path}).is_some()
    }
    pub fn add_file(&mut self, file:NewFile, in_folder:Option<&mut ProxFolder>) -> Option<FileID> {
        if !self.file_exists(file.absolute_path.clone()) {
            match file.absolute_path.file_name() {
                Some(name) => {
                    let name = name.to_os_string().to_string_lossy().to_string();
                    let extension = match file.absolute_path.extension() {
                        Some(ext) => {
                            Some(ext.to_os_string().to_string_lossy().to_string())
                        },
                        None => None
                    };
                    let id = self.take_next_file();
                    self.all_files.push(ProxFile {access_modes:HashSet::from([0]), id, absolute_path:file.absolute_path, tags:HashSet::new(), desc:None, name, extension, from_device:file.from_device, added_at:Utc::now(), last_modified:Utc::now()});
                    match in_folder {
                        Some(fold) => {
                            fold.add_file_child(id);
                        },
                        None => ()
                    }
                    Some(id)
                },
                None => panic!("Tried to add a file but it wasn't a file"),
            }
        }
        else {
            None
        }
        
    }
    pub fn remove_file(&mut self, id:FileID, folders:&mut Folders) {
        todo!("implement ProxFile removal")
    }
    pub fn get_last_file(&self) -> Option<&ProxFile> {
        self.all_files.last()
    }
    pub fn add_file_raw(&mut self, mut file:ProxFile) -> FileID {
        let file_id = self.all_files.len();
        file.id = file_id;
        self.all_files.push(file);
        file_id
    }
    pub fn get_file_by_path(&self, path:AbsolutePath) -> Option<FileID> {
        self.all_files.iter().find(|file| {file.absolute_path == path}).and_then(|file|{Some(file.id)})
    }
}