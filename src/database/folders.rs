use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs::read_dir;
use std::io::{Error, ErrorKind};
use std::{fs::ReadDir, path::PathBuf};
use std::os;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::access_modes::AccessModeID;
use super::devices::DeviceID;
use super::files::{Files, NewFile};
use super::{description::Description, files::FileID, tags::TagID};

pub type AbsolutePath = PathBuf;
pub type RelativePath = PathBuf;
pub type FolderID = usize;
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProxFolder {
    id:FolderID,
    absolute_path:AbsolutePath,
    tags:HashSet<TagID>,
    desc:Option<Description>,
    name:String,
    last_updated:Option<DateTime<Utc>>,
    recursive:RecursivityLevel,
    children:Vec<FolderID>,
    parent:Option<FolderID>,
    files:Vec<FileID>,
    from_device:DeviceID,
    access_modes:HashSet<AccessModeID>
}

impl ProxFolder {
    pub fn new_empty(id:FolderID, absolute_path:AbsolutePath, parent:Option<FolderID>, recursive:RecursivityLevel, from_device:DeviceID) -> Self {
        Self {access_modes:HashSet::from([0]), id, from_device, absolute_path:absolute_path.clone(), tags: HashSet::with_capacity(4), desc: None, name:absolute_path.file_name().unwrap().to_string_lossy().to_string() , last_updated:None, recursive, children: Vec::with_capacity(4), parent, files: Vec::with_capacity(4) }
    }
    pub fn get_id(&self) -> FolderID {
        self.id
    }
    pub fn add_file_child(&mut self, file_id:FileID) {
        self.files.push(file_id);
    }
    pub fn add_desc_tags(&mut self, desc:Description, tags:HashSet<TagID>) {
        self.desc = Some(desc);
        self.tags = tags;
    }
    pub fn get_name_string(&self) -> String {
        self.name.clone()
    }
    pub fn get_desc(&self) -> Option<Description> {
        self.desc.clone()
    }
    pub fn get_full_path(&self) -> AbsolutePath {
        self.absolute_path.clone()
    }
    pub fn get_folder_children(&self) -> &Vec<FolderID> {
        &self.children
    }
    pub fn get_file_children(&self) -> &Vec<FileID> {
        &self.files
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecursivityLevel {
    No,
    Infinite,
    Fixed(usize)
}

impl PartialOrd for RecursivityLevel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RecursivityLevel {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self {
            RecursivityLevel::No => match other {
                RecursivityLevel::No => Ordering::Equal,
                _ => Ordering::Less
            },
            RecursivityLevel::Fixed(value1) => match other {
                RecursivityLevel::No => Ordering::Greater,
                RecursivityLevel::Fixed(value2) => value1.cmp(value2),
                RecursivityLevel::Infinite => Ordering::Less, 
            },
            RecursivityLevel::Infinite => match other {
                RecursivityLevel::Infinite => Ordering::Equal,
                _ => Ordering::Greater,
            }
        }   
    }
}

pub struct NewFolder {
    absolute_path:AbsolutePath,
    recursivity:RecursivityLevel,
    from_device:DeviceID
}

impl NewFolder {
    pub fn new(absolute_path:AbsolutePath, recursivity:RecursivityLevel, from_device:DeviceID) -> Self {
        Self { absolute_path, recursivity, from_device }
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Folders {
    all_folders:HashMap<FolderID, ProxFolder>,
    path_to_id_map:HashMap<AbsolutePath, FolderID>,
    starting_points:HashSet<FolderID>,
    ignore_those_folders:HashSet<AbsolutePath>,
    ignore_those_folder_names:HashSet<String>,
    ignore_those_file_extensions:HashSet<String>,
    latest_id:FolderID,
}

impl Folders {
    pub fn get_folder_by_id(&self, id:FolderID) -> &ProxFolder {
        self.all_folders.get(&id).unwrap()
    }
    pub fn new() -> Self {
        Self { all_folders: HashMap::with_capacity(1024), starting_points:HashSet::with_capacity(1024), path_to_id_map:HashMap::with_capacity(1024), latest_id:0, ignore_those_folders:HashSet::with_capacity(1024),ignore_those_folder_names:HashSet::with_capacity(1024), ignore_those_file_extensions:HashSet::with_capacity(1024) }
    }
    pub fn get_folder_mut(&mut self, folder:FolderID) -> &mut ProxFolder {
        self.all_folders.get_mut(&folder).unwrap()
    }

    pub fn number_of_folders(&self) -> usize {
        self.all_folders.len()
    }
    pub fn add_to_ignore_list(&mut self, folders:Vec<AbsolutePath>) {
        for folder in folders {
            self.ignore_those_folders.insert(folder);
        }
    }
    pub fn add_to_folder_name_ignore_list(&mut self, names:Vec<String>) {
        for name in names {
            self.ignore_those_folder_names.insert(name);
        }
    }
    pub fn add_to_extension_ignore_list(&mut self, extensions:Vec<String>) {
        for ext in extensions {
            self.ignore_those_file_extensions.insert(ext);
        }
    }
    pub fn get_parent_to(&self, path:AbsolutePath) -> Option<FolderID> {
        match path.parent() {
            Some(parent) => {
                match self.all_folders.iter().find(|(folder_id, folder)| {folder.absolute_path == parent}) {
                    Some((folder_id, folder)) => Some(folder_id.clone()),
                    None => None,
                }
            },
            None => None,
        }
    }
    pub fn start_down_this_folder(&mut self, new_folder:NewFolder, files:&mut Files) -> Result<FolderID, Error> {
        let parent = self.get_parent_to(new_folder.absolute_path.clone());
        let id = self.add_folder(new_folder, parent, files)?;
        self.starting_points.insert(id);
        Ok(id)
    }
    pub fn get_folder_if_already_exists(&self, path:AbsolutePath) -> Option<FolderID> {
        self.path_to_id_map.get(&path).copied().to_owned()
    }
    pub fn add_folder(&mut self, mut new_folder:NewFolder, parent_id:Option<FolderID>,files:&mut Files) -> Result<FolderID, Error> {
        if !self.ignore_those_folders.contains(&new_folder.absolute_path) && !self.ignore_those_folder_names.contains(&new_folder.absolute_path.file_name().unwrap().to_string_lossy().to_string().trim_matches('/').to_string()) {
            let id = match self.get_folder_if_already_exists(new_folder.absolute_path.clone()) {
                Some(folder_id) => {
                    let mut old_folder = self.all_folders.get_mut(&folder_id).unwrap();
                    if old_folder.parent.is_some() && old_folder.parent != parent_id {
                        panic!("parent not matching expected parent");
                    }
                    new_folder.recursivity = old_folder.recursive.max(new_folder.recursivity);
                    folder_id
                },
                None => {
                    let id = self.take_next_folder();
                    self.all_folders.insert(id, ProxFolder::new_empty(id, new_folder.absolute_path.clone(), parent_id, new_folder.recursivity.clone(), new_folder.from_device));
                    self.path_to_id_map.insert(new_folder.absolute_path.clone(), id);
                    id
                }
            };
            println!("Exploring {}", new_folder.absolute_path.clone().to_string_lossy().to_string());
            match new_folder.recursivity {
                RecursivityLevel::No => {
                    self.complete_folder(new_folder, id, parent_id)
                },
                RecursivityLevel::Infinite => {
                    for entry in read_dir(&new_folder.absolute_path)? {
                        let ent = entry?;
                        let file_type = ent.metadata()?.file_type();
                        if file_type.is_dir() {
                            let new_new_folder = NewFolder {recursivity:RecursivityLevel::Infinite,
                                absolute_path:PathBuf::from(ent.path().to_str().expect("That path is not unicode, nuh uh")),
                                from_device:new_folder.from_device
                            };
                            self.add_folder(new_new_folder, Some(id), files);
                        }
                        else if file_type.is_file() {
                            let new_file = NewFile::new(PathBuf::from(ent.path().to_str().expect("That path is not unicode, nuh uh")), new_folder.from_device);
                            files.add_file(new_file, Some(self.all_folders.get_mut(&id).unwrap()));
                        }
                    }
                    self.complete_folder(new_folder, id, parent_id)
                },
                RecursivityLevel::Fixed(levels_left) => {
                    if levels_left > 0 {
                        for entry in read_dir(&new_folder.absolute_path)? {
                            let ent = entry?;
                            let file_type = ent.metadata()?.file_type();
                            if file_type.is_dir() {
                                let new_new_folder = NewFolder {recursivity:RecursivityLevel::Fixed(levels_left - 1),
                                    absolute_path:PathBuf::from(new_folder.absolute_path.to_str().expect("That path is not unicode, nuh uh").to_string() + ent.path().to_str().expect("That path is not unicode, nuh uh")),
                                    from_device:new_folder.from_device
                                };
                                self.add_folder(new_new_folder, Some(id), files);
                            }
                            else if file_type.is_file() {
                                let new_file = NewFile::new(PathBuf::from(ent.path().to_str().expect("That path is not unicode, nuh uh")), new_folder.from_device);
                                files.add_file(new_file, Some(self.all_folders.get_mut(&id).unwrap()));
                            }
                        }
                    }
                    self.complete_folder(new_folder, id, parent_id)
                }
            }
        }
        else {
            Err(Error::new(ErrorKind::Other, "Ignored Folder found"))
        }
        
        
    }
    fn take_next_folder(&mut self) -> FolderID {
        let id = self.latest_id;
        self.latest_id += 1;
        id
    }
    fn complete_folder(&mut self, new_folder:NewFolder, id:FolderID, parent_id:Option<FolderID>) -> Result<FolderID, Error> {
        match parent_id {
            Some(parent) => {
                let mut parent_folder = self.all_folders.get_mut(&parent).unwrap();
                parent_folder.children.push(id);
                Ok(id)
            },
            None => {
                Ok(id)
            }
        }
    }
    pub fn add_folder_raw(&mut self, mut folder:ProxFolder) -> FolderID {
        let id = self.all_folders.len();
        folder.id = id;
        self.all_folders.insert(id, folder);
        id
    }
}