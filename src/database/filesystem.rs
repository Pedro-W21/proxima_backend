use std::{collections::{HashMap, HashSet}, f32::consts::E, fs::{self, DirBuilder, File, read_dir}, io::{self, Read, Write}, path::PathBuf, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::database::{access_modes::AccessModeID, devices::DeviceID};


#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProximaPath {
    device:DeviceID,
    on_device_path:Vec<FSElementID>,
}

impl ProximaPath {
    fn new(device:DeviceID, on_device_path:Vec<FSElementID>) -> Self {
        Self { device, on_device_path }
    }
    pub fn get_device(&self) -> DeviceID {
        self.device
    }
    pub fn get_on_device_path(&self) -> &Vec<FSElementID> {
        &self.on_device_path
    }
    pub fn join(&self, element:FSElementID) -> Self {
        let mut new_path = self.clone();
        new_path.on_device_path.push(element);
        new_path
    }
    pub fn parent(&self) -> Self {
        let mut new_path = self.clone();
        if new_path.on_device_path.len() > 0 {
            new_path.on_device_path.remove(new_path.on_device_path.len() - 1);
            new_path
        }
        else {
            panic!("Getting parent of empty path")
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filesystem {
   device_filesystems:HashMap<DeviceID, DeviceFilesystem>,
   id_counter:usize,
}

#[derive(Debug)]
pub enum ProxFilesystemError {
    NonExistentDevice{device_target:String},
    ExitingFilesystem,
    NonEmptyFolder,
    MovingIntoFile,
    ElementNotFound {element:String},
    LocalElementNotFound {element:String},
    LocalIOError {error:io::Error},
    LocalElementNotFile,
    ElementAlreadyExists {name:String},
    PermissionDenied
}

#[derive(Clone, Copy)]
pub enum PathCreation {
    IntoFolder,
    IntoFile
}

impl Filesystem {
    pub fn resolve_existing_path(&self, path_str:String, working_directory:Option<&ProximaPath>) -> Result<ProximaPath, ProxFilesystemError> {
        let (mut new_path, to_skip, mut current_element) = match working_directory {
            Some(wd) => (wd.clone(), 0, Some(wd.get_on_device_path().last().cloned().unwrap_or(self.device_filesystems.get(&wd.get_device()).unwrap().root_element))),
            None => {
                let first_split = path_str.split("/").skip(1).next().ok_or(ProxFilesystemError::NonExistentDevice { device_target: String::from("a") })?;
                let target_device = first_split.parse().or(Err(ProxFilesystemError::NonExistentDevice { device_target: first_split.to_string() }))?;

                (ProximaPath { device:target_device , on_device_path: Vec::with_capacity(8) }, 2, Some(self.device_filesystems.get(&target_device).unwrap().root_element))
            }
        };
        'parsing: for part in path_str.split("/").skip(to_skip) {
            match current_element {
                Some(elem) => {
                    let device = self.device_filesystems.get(&new_path.device).unwrap();
                    let element = device.elements.get(&elem).unwrap();

                    match &element.element_type {
                        FSElementType::File => return Err(ProxFilesystemError::MovingIntoFile),
                        FSElementType::Folder { children } => {
                            if part == ".." {
                                current_element = element.parent;
                                new_path.on_device_path.remove(new_path.on_device_path.len() - 1);
                            }
                            else {
                                for child in children {
                                    if &device.elements.get(child).unwrap().name == part {
                                        current_element = Some(*child);
                                        new_path.on_device_path.push(*child);
                                        continue 'parsing;
                                    }
                                }
                                return Err(ProxFilesystemError::ElementNotFound { element: part.to_string() })
                            }
                        }
                    }
                },
                None => {
                    let target_device = part.parse::<usize>().or(Err(ProxFilesystemError::NonExistentDevice { device_target: part.to_string() }))?;
                    if let Some(device) = self.device_filesystems.get(&target_device) {
                        new_path.device = target_device;
                        current_element = Some(device.root_element);
                    }   
                    else {
                        return Err(ProxFilesystemError::NonExistentDevice { device_target: part.to_string() })
                    }
                }
            }
        }
        Ok(new_path)
        
    }
    pub fn resolve_new_path(&mut self, path_str:String, working_directory:Option<&ProximaPath>, creation:FSElementType, access_mode:AccessModeID, specific_perms:Permissions) -> Result<ProximaPath, ProxFilesystemError> {
        let (mut new_path, to_skip, mut current_element) = match working_directory {
            Some(wd) => (wd.clone(), 0, Some(wd.get_on_device_path().last().cloned().unwrap_or(self.device_filesystems.get(&wd.get_device()).unwrap().root_element))),
            None => {
                let first_split = path_str.split("/").skip(1).next().ok_or(ProxFilesystemError::NonExistentDevice { device_target: String::from("a") })?;
                let target_device = first_split.parse().or(Err(ProxFilesystemError::NonExistentDevice { device_target: first_split.to_string() }))?;

                (ProximaPath { device:target_device , on_device_path: Vec::with_capacity(8) }, 2, Some(self.device_filesystems.get(&target_device).unwrap().root_element))
            }
        };
        let parts:Vec<&str> = path_str.split("/").skip(to_skip).collect();
        'parsing: for (i, part) in parts.iter().enumerate() {
            match current_element {
                Some(elem) => {
                    let mut element = self.device_filesystems.get(&new_path.device).unwrap().elements.get(&elem).unwrap().clone();

                    match &mut element.element_type {
                        FSElementType::File => return Err(ProxFilesystemError::MovingIntoFile),
                        FSElementType::Folder { children } => {
                            if *part == ".." {
                                current_element = element.parent;
                                new_path.on_device_path.remove(new_path.on_device_path.len() - 1);
                            }
                            else {
                                for child in children.iter() {
                                    if &self.device_filesystems.get(&new_path.device).unwrap().elements.get(child).unwrap().name == *part {
                                        current_element = Some(*child);
                                        new_path.on_device_path.push(*child);
                                        continue 'parsing;
                                    }
                                }
                                let new_element = self.create(&new_path, part.to_string(), if i == parts.len() - 1 {creation.clone()} else {FSElementType::Folder { children: Vec::with_capacity(4) }}, FSPermissions::new_with_am_specific(Permissions::new(true, false), access_mode, specific_perms.clone()), access_mode)?;
                                children.push(new_element);
                                self.device_filesystems.get_mut(&new_path.device).unwrap().elements.insert(elem, element);
                                new_path.on_device_path.push(new_element);
                            }
                        }
                    }
                },
                None => {
                    let target_device = part.parse::<usize>().or(Err(ProxFilesystemError::NonExistentDevice { device_target: part.to_string() }))?;
                    if let Some(device) = self.device_filesystems.get(&target_device) {
                        new_path.device = target_device;
                        current_element = Some(device.root_element);
                    }   
                    else {
                        return Err(ProxFilesystemError::NonExistentDevice { device_target: part.to_string() })
                    }
                }
            }
        }
        Ok(new_path)
        
    }
    pub fn get_at(&self, path:&ProximaPath, access_mode:AccessModeID) -> Result<&FilesystemElement, ProxFilesystemError> {
        match self.device_filesystems.get(&path.get_device()) {
            Some(device) => match device.elements.get(path.on_device_path.last().unwrap_or(&device.root_element)) {
                Some(element) => if element.can_read(access_mode) {
                    Ok(element)
                }
                else {
                    Err(ProxFilesystemError::PermissionDenied)
                },
                None => Err(ProxFilesystemError::ElementNotFound { element: format!("element not found") })
            },
            None => Err(ProxFilesystemError::NonExistentDevice { device_target: format!("{}", &path.get_device()) })
        }
    }
    pub fn get_at_mut(&mut self, path:&ProximaPath, access_mode:AccessModeID) -> Result<&mut FilesystemElement, ProxFilesystemError> {
        match self.device_filesystems.get_mut(&path.get_device()) {
            Some(device) => match device.elements.get_mut(path.on_device_path.last().unwrap_or(&device.root_element)) {
                Some(element) => if element.can_read(access_mode) {
                    Ok(element)
                }
                else {
                    Err(ProxFilesystemError::PermissionDenied)
                },
                None => Err(ProxFilesystemError::ElementNotFound { element: format!("element not found") })
            },
            None => Err(ProxFilesystemError::NonExistentDevice { device_target: format!("{}", &path.get_device()) })
        }
    }
    pub fn path_on_device(&self, path:&ProximaPath) -> Result<String, ProxFilesystemError> {
        let mut final_path = String::with_capacity(32);
        match self.device_filesystems.get(&path.get_device()) {
            Some(dev) => {
                let mut current_elem = dev.elements.get(&dev.root_element).unwrap();
                final_path += &dev.root_path;
                'conversion: for part in path.get_on_device_path() {
                    match &current_elem.element_type {
                        FSElementType::Folder { children } => for child in children {
                            if *child == *part {
                                current_elem = dev.elements.get(child).unwrap();
                                final_path += &format!("/{}", current_elem.name);
                                continue 'conversion;
                            }
                        },
                        FSElementType::File => return Err(ProxFilesystemError::MovingIntoFile) 
                    }
                }
            },
            None => return Err(ProxFilesystemError::NonExistentDevice { device_target: format!("{}", &path.get_device()) })
        }
        Ok(final_path)
    }
    pub fn create(&mut self, parent_path:&ProximaPath, name:String, element_type:FSElementType, permissions:FSPermissions, access_mode:AccessModeID) -> Result<FSElementID, ProxFilesystemError> {
        let element_id = self.id_counter;
        self.id_counter += 1;
        let element = if parent_path.get_device() == 0 {
            create_on_device(element_id, parent_path.get_on_device_path().last().cloned(), element_type, self.path_on_device(parent_path)?, name, permissions)?
        }
        else {
            todo!("support creating files on non-server devices")
        };
        self.get_at_mut(parent_path, access_mode)?.get_children_mut().unwrap().push(element_id);
        self.device_filesystems.get_mut(&parent_path.get_device()).unwrap().elements.insert(element_id, element);
        Ok(element_id)
    }
    pub fn list(&mut self, target:&ProximaPath, access_mode:AccessModeID) -> Result<FSList, ProxFilesystemError> {
        let device_list = if target.get_device() == 0 {
            list_on_device(self.path_on_device(target)?)?
        }
        else {
            todo!("implement listing on other devices")
        };
        let server_list = self.get_at(target, access_mode)?.get_children().unwrap().iter().map(|child| {
            let child_elem = self.device_filesystems.get(&target.get_device()).unwrap().elements.get(child).unwrap();
            ((child_elem.name.clone(), child_elem.element_type.clone()), *child)
        }).collect::<HashMap<(String, FSElementType), FSElementID>>();
        let mut list = FSList {device:target.get_device(), parent:self.get_at(target,access_mode)?.id, elements:Vec::with_capacity(device_list.len())};
        for child_elem in device_list {
            let element_id = if !server_list.contains_key(&child_elem) {
                self.create(target, child_elem.0, child_elem.1, FSPermissions::new_with_am_specific(Permissions::new(true, false), access_mode, Permissions::new(true, true)), access_mode)?
            }
            else {
                *server_list.get(&child_elem).unwrap()
            };
            list.elements.push(element_id);
        }
        Ok(list)
    }
    pub fn read(&mut self, target:&ProximaPath, options:ReadOptions, access_mode:AccessModeID) -> Result<FSRead, ProxFilesystemError> {
        let element = self.get_at(target, access_mode)?;
        match &element.element_type {
            FSElementType::File => if target.get_device() == 0  {
                read_on_device(self.path_on_device(target)?, options)
            }
            else {
                todo!("implement reading files on other devices")
            },
            FSElementType::Folder { children } => Ok(FSRead::FolderRead { list: self.list(target, access_mode)? })
        }
    }
    pub fn delete(&mut self, target:&ProximaPath, recursive:bool, access_mode:AccessModeID) -> Result<Vec<FSElementID>, ProxFilesystemError> {
        let mut deleted = Vec::with_capacity(2);
        let element = self.get_at(target, access_mode)?.clone();
        match &element.element_type {
            FSElementType::File => if target.get_device() == 0  {

                delete_on_device(self.path_on_device(target)?)?;
                deleted.push(element.id);
                self.get_at_mut(&target.parent(), access_mode)?.get_children_mut().unwrap().retain_mut(|elem| {*elem != element.id});
                self.device_filesystems.get_mut(&target.get_device()).unwrap().elements.remove(&element.id);
                Ok(deleted)
            }
            else {
                todo!("implement reading files on other devices")
            },
            FSElementType::Folder { children } => {
                if children.len() > 0 {
                    if recursive {
                        for child in children {
                            let mut child_deleted = self.delete(&target.join(*child), true, access_mode)?;
                            deleted.append(&mut child_deleted);
                        }
                        delete_on_device(self.path_on_device(target)?)?;
                        deleted.push(element.id);
                        self.get_at_mut(&target.parent(), access_mode)?.get_children_mut().unwrap().retain_mut(|elem| {*elem != element.id});
                        self.device_filesystems.get_mut(&target.get_device()).unwrap().elements.remove(&element.id);
                        Ok(deleted)
                    }
                    else {
                        Err(ProxFilesystemError::NonEmptyFolder)
                    }
                }
                else {
                    delete_on_device(self.path_on_device(target)?)?;
                    deleted.push(element.id);
                    self.get_at_mut(&target.parent(), access_mode)?.get_children_mut().unwrap().retain_mut(|elem| {*elem != element.id});
                    self.device_filesystems.get_mut(&target.get_device()).unwrap().elements.remove(&element.id);
                    Ok(deleted)
                }
                
                
            },
        }
    }
    pub fn write(&self, target:&ProximaPath, data:Vec<u8>, access_mode:AccessModeID) -> Result<(), ProxFilesystemError> {
        let element = self.get_at(target, access_mode)?;
        match &element.element_type {
            FSElementType::File => if target.get_device() == 0 {
                write_on_device(self.path_on_device(target)?, data)
            }
            else {
                todo!("implement writing on other devices")
            },
            FSElementType::Folder { children } => Err(ProxFilesystemError::LocalElementNotFile)
        }
    }
    fn copy_same_device(&mut self, source:&ProximaPath, destination:&ProximaPath, access_mode:AccessModeID) -> Result<Vec<FSElementID>, ProxFilesystemError> {
        let mut new_ids = Vec::new();
        let source_elem = self.get_at(source, access_mode)?.clone();
        match &source_elem.element_type {
            FSElementType::File => {
                let dest_elem = self.get_at(destination, access_mode)?.clone();
                match &dest_elem.element_type {
                    FSElementType::File => if source.get_device() == 0 {
                        new_ids.push(dest_elem.id);
                        copy_file_on_device(self.path_on_device(source)?, self.path_on_device(destination)?)?;
                        Ok(new_ids)
                    }
                    else {
                        todo!("implement copying files on same device on non-server device")
                    },
                    FSElementType::Folder { children } => if children.iter().any(|child| {self.get_at(&destination.join(*child), access_mode).unwrap().name == source_elem.name}) {
                        Err(ProxFilesystemError::ElementAlreadyExists { name: source_elem.name.clone() })
                    }
                    else if source.get_device() == 0 {
                        let new_id = self.create(destination, source_elem.name.clone(), FSElementType::File, source_elem.permissions.clone(), access_mode)?;
                        new_ids.push(new_id);
                        copy_file_on_device(self.path_on_device(source)?, self.path_on_device(&destination.join(new_id))?)?;
                        Ok(new_ids)
                    }
                    else {
                        todo!("implement copying files into folder on same device on non-server device")
                    }
                }
            },
            FSElementType::Folder { children:source_children } => {
                let dest_elem = self.get_at(destination, access_mode)?.clone();
                match &dest_elem.element_type {
                    FSElementType::File => Err(ProxFilesystemError::MovingIntoFile),
                    FSElementType::Folder { children:dest_children } => {
                        let source_file_names = source_children.iter().map(|child| {self.get_at(&destination.join(*child), access_mode).unwrap().name.clone()}).collect::<HashSet<String>>();
                        if dest_children.iter().any(|child| {source_file_names.contains(&self.get_at(&destination.join(*child), access_mode).unwrap().name)}) {
                            Err(ProxFilesystemError::ElementAlreadyExists { name: source_elem.name.clone() })
                        }
                        else {
                            for child in source_children {
                                let elem = self.get_at(&source.join(*child), access_mode)?;
                                let new_id = self.create(destination, elem.name.clone(), elem.element_type.clone_empty(), elem.permissions.clone(), access_mode)?;
                                new_ids.push(new_id);
                                let mut child_new_ids = self.copy_same_device(&source.join(*child), &destination.join(new_id), access_mode)?;
                                new_ids.append(&mut child_new_ids);
                            }
                            Ok(new_ids)
                        }
                    }
                }
            }
        }
    }
    fn copy_different_devices(&mut self, source:&ProximaPath, destination:&ProximaPath, access_mode:AccessModeID) -> Result<Vec<FSElementID>, ProxFilesystemError> {
        let mut new_ids = Vec::new();
        let source_elem = self.get_at(source, access_mode)?.clone();
        match &source_elem.element_type {
            FSElementType::File => {
                let dest_elem = self.get_at(destination, access_mode)?.clone();
                match &dest_elem.element_type {
                    FSElementType::File => {
                        new_ids.push(dest_elem.id);
                        let data = self.read(source, ReadOptions { line_numbering: false }, access_mode)?.get_binary().unwrap();
                        self.write(destination, data, access_mode)?;
                        Ok(new_ids)
                    },
                    FSElementType::Folder { children } => if children.iter().any(|child| {self.get_at(&destination.join(*child), access_mode).unwrap().name == source_elem.name}) {
                        Err(ProxFilesystemError::ElementAlreadyExists { name: source_elem.name.clone() })
                    }
                    else {
                        let new_id = self.create(destination, source_elem.name.clone(), FSElementType::File, source_elem.permissions.clone(), access_mode)?;
                        new_ids.push(new_id);
                        let data = self.read(source, ReadOptions { line_numbering: false }, access_mode)?.get_binary().unwrap();
                        self.write(destination, data, access_mode)?;
                        Ok(new_ids)
                    }
                }
            },
            FSElementType::Folder { children:source_children } => {
                let dest_elem = self.get_at(destination, access_mode)?.clone();
                match &dest_elem.element_type {
                    FSElementType::File => Err(ProxFilesystemError::MovingIntoFile),
                    FSElementType::Folder { children:dest_children } => {
                        let source_file_names = source_children.iter().map(|child| {self.get_at(&destination.join(*child), access_mode).unwrap().name.clone()}).collect::<HashSet<String>>();
                        if dest_children.iter().any(|child| {source_file_names.contains(&self.get_at(&destination.join(*child), access_mode).unwrap().name)}) {
                            Err(ProxFilesystemError::ElementAlreadyExists { name: source_elem.name.clone() })
                        }
                        else {
                            for child in source_children {
                                let elem = self.get_at(&source.join(*child), access_mode)?;
                                let new_id = self.create(destination, elem.name.clone(), elem.element_type.clone_empty(), elem.permissions.clone(), access_mode)?;
                                new_ids.push(new_id);
                                let mut child_new_ids = self.copy_different_devices(&source.join(*child), &destination.join(new_id), access_mode)?;
                                new_ids.append(&mut child_new_ids);
                            }
                            Ok(new_ids)
                        }
                    }
                }
            }
        }
    }
    pub fn move_copy(&mut self, source:&ProximaPath, destination:&ProximaPath, copy:bool, access_mode:AccessModeID) -> Result<Vec<FSElementID>, ProxFilesystemError> {
        let new_ids = if source.get_device() == destination.get_device() {
            self.copy_same_device(source, destination, access_mode)?
        }
        else {
            self.copy_different_devices(source, destination, access_mode)?
        };
        if !copy {
            self.delete(source, true, access_mode)?;
        }
        Ok(new_ids)

    }
}

pub struct ReadOptions {
    line_numbering:bool,
}

pub struct FSList {
    device:DeviceID,
    parent:FSElementID,
    elements:Vec<FSElementID>
}

pub enum FSRead {
    FolderRead {list:FSList},
    TextFileRead {file:String},
    BinaryFileRead {binary:Vec<u8>}
}

impl FSRead {
    pub fn get_binary(self) -> Option<Vec<u8>> {
        match self {
            Self::FolderRead { list } => None,
            Self::BinaryFileRead { binary } => Some(binary),
            Self::TextFileRead { file } => Some(file.into_bytes())
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceFilesystem {
    elements:HashMap<FSElementID, FilesystemElement>,
    root_element:FSElementID,
    root_path:String,
}

impl DeviceFilesystem {

}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemElement {
    created_on:DateTime<Utc>,
    id:FSElementID,
    parent:Option<FSElementID>,
    element_type:FSElementType,
    permissions:FSPermissions,
    name:String,
}

impl FilesystemElement {
    pub fn get_children_mut(&mut self) -> Option<&mut Vec<FSElementID>> {
        match &mut self.element_type {
            FSElementType::File => None, 
            FSElementType::Folder { children } => Some(children)
        }
    }
    pub fn get_children(&self) -> Option<&Vec<FSElementID>> {
        match &self.element_type {
            FSElementType::File => None, 
            FSElementType::Folder { children } => Some(children)
        }
    }
    pub fn can_read(&self, access_mode:AccessModeID) -> bool {
        self.permissions.am_specific.get(&access_mode).unwrap_or(&self.permissions.general).can_read()
    }
    pub fn can_write(&self, access_mode:AccessModeID) -> bool {
        self.permissions.am_specific.get(&access_mode).unwrap_or(&self.permissions.general).can_write()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum FSElementType {
    File,
    Folder {children:Vec<FSElementID>}
}

impl FSElementType {
    pub fn clone_empty(&self) -> Self {
        match self {
            Self::File => Self::File,
            Self::Folder { children } => Self::Folder { children: Vec::with_capacity(2) }
        }
    }
}

pub type FSElementID = usize;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FSPermissions {
    am_specific:HashMap<AccessModeID, Permissions>,
    general:Permissions
}

impl FSPermissions {
    pub fn new(general:Permissions) -> Self {
        Self { am_specific: HashMap::with_capacity(2), general }
    }
    pub fn new_with_am_specific(general:Permissions, am:AccessModeID, specific:Permissions) -> Self {
        Self { am_specific: HashMap::from([(am, specific)]), general }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Permissions {
    read:bool,
    write:bool,
}

impl Permissions {
    pub fn new(read:bool, write:bool) -> Self {
        Self { read, write }
    }
    pub fn can_read(&self) -> bool {
        self.read
    }
    pub fn can_write(&self) -> bool {
        self.write
    }
}

pub fn create_on_device(element_id:FSElementID, parent:Option<FSElementID>, element_type:FSElementType, parent_path_on_device:String, name:String, permissions:FSPermissions) -> Result<FilesystemElement, ProxFilesystemError> {
    let local_parent_path = PathBuf::from_str(&parent_path_on_device).unwrap();
    if local_parent_path.is_dir() || local_parent_path.is_symlink() {
        let local_target_path = local_parent_path.join(name.clone());
        match &element_type {
            FSElementType::File => match File::create(local_target_path) {
                Ok(target) => Ok(FilesystemElement { created_on: Utc::now(), id: element_id, parent, element_type, name, permissions }),
                Err(err) => Err(ProxFilesystemError::LocalIOError { error: err })
            },
            FSElementType::Folder { children } => match DirBuilder::new().create(local_target_path) {
                Ok(target) => Ok(FilesystemElement { created_on: Utc::now(), id: element_id, parent, element_type, name, permissions }),
                Err(err) => Err(ProxFilesystemError::LocalIOError { error: err })
            }
        }
    }
    else {
        Err(ProxFilesystemError::LocalElementNotFound { element: parent_path_on_device })
    }
    
}

pub fn list_on_device(path_on_device:String) -> Result<Vec<(String, FSElementType)>, ProxFilesystemError> {
    let local_target_path = PathBuf::from_str(&path_on_device).unwrap();
    let mut out = Vec::with_capacity(4);
    if local_target_path.is_dir() {
        match read_dir(path_on_device) {
            Ok(dir) => for elem in dir {
                match elem {
                    Ok(entry) => if entry.path().is_dir() {
                        out.push((entry.path().iter().last().unwrap().to_string_lossy().to_string(), FSElementType::Folder { children: Vec::new() }));
                    }
                    else if entry.path().is_file() {
                        out.push((entry.path().iter().last().unwrap().to_string_lossy().to_string(), FSElementType::File));
                    },
                    Err(err) => return Err(ProxFilesystemError::LocalIOError { error: err })
                }

            },
            Err(err) => return Err(ProxFilesystemError::LocalIOError { error: err })
        }
        Ok(out)
    }
    else {
        Err(ProxFilesystemError::LocalElementNotFound { element: path_on_device })
    }
}

pub fn read_on_device(path_on_device:String, options:ReadOptions) -> Result<FSRead, ProxFilesystemError> {
    let local_target_path = PathBuf::from_str(&path_on_device).unwrap();
    if local_target_path.is_file() {
        let mut file = fs::File::open(local_target_path).map_err(|err| {ProxFilesystemError::LocalIOError { error: err }})?;
        let mut contents = Vec::with_capacity(1024 * 64);
        match file.read_to_end(&mut contents) {
            Ok(read) => Ok(
                match str::from_utf8(&contents) {
                    Ok(text) => FSRead::TextFileRead { file: text.to_string() },
                    Err(_) => FSRead::BinaryFileRead { binary: contents }
                }
            ),
            Err(err) => Err(ProxFilesystemError::LocalIOError { error: err })
        }
        // TODO : implement binary file read
    }
    else {
        Err(ProxFilesystemError::LocalElementNotFile)
    }
}

pub fn delete_on_device(path_on_device:String) -> Result<(), ProxFilesystemError> {
    let local_target_path = PathBuf::from_str(&path_on_device).unwrap();
    if local_target_path.is_file() {
        fs::remove_file(local_target_path).map_err(|err| {ProxFilesystemError::LocalIOError { error: err }})
    }
    else if local_target_path.is_dir() {
        fs::remove_dir(local_target_path).map_err(|err| {ProxFilesystemError::LocalIOError { error: err }})
    }
    else {
        Err(ProxFilesystemError::LocalElementNotFile)
    }
}

pub fn write_on_device(path_on_device:String, content:Vec<u8>) -> Result<(), ProxFilesystemError> {
    let local_target_path = PathBuf::from_str(&path_on_device).unwrap();
    let mut file = fs::File::open(local_target_path).map_err(|err| {ProxFilesystemError::LocalIOError { error: err }})?;
    file.write_all(&content).map_err(|err| {ProxFilesystemError::LocalIOError { error: err }})
}

pub fn copy_file_on_device(source_path:String, dest_path:String) -> Result<(), ProxFilesystemError> {
    let local_source_path = PathBuf::from_str(&source_path).unwrap();
    let local_dest_path = PathBuf::from_str(&dest_path).unwrap();

    fs::copy(local_source_path, local_dest_path).map_err(|err| {ProxFilesystemError::LocalIOError { error: err }}).map(|val| {})
}