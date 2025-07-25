use std::{collections::{HashMap, HashSet, VecDeque}, iter::Step, path::PathBuf, sync::{mpmc::{channel, Receiver, Sender}, mpsc::{RecvTimeoutError, SendError}}, thread, time::Duration};

use access_modes::{AccessMode, AccessModeID, AccessModes};
use chats::{Chat, ChatID, Chats};
use description::{Description, DescriptionTarget};
use devices::{Device, DeviceID, Devices};
use files::{FileID, Files, ProxFile};
use folders::{FolderID, Folders, ProxFolder};
use loading_saving::create_or_repair_database_folder_structure;
use rand::{rngs::StdRng, RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use tags::{Tag, TagID, Tags};
use user::{PersonalInformation, UserData};

use crate::{ai_interaction::create_prompt::{get_agent_prompt_context, AgentPrompt}, database::{configuration::{ChatConfigID, ChatConfiguration, ChatConfigurations}, context::WholeContext, loading_saving::{load_from_disk, save_to_disk}}};

pub mod tags;
pub mod folders;
pub mod description;
pub mod files;
pub mod user;
pub mod chats;
pub mod loading_saving;
pub mod context;
pub mod devices;
pub mod access_modes;
pub mod configuration;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProxDatabase {
    pub files:Files,
    pub folders:Folders,
    pub chats:Chats,
    pub tags:Tags,
    pub personal_info:PersonalInformation,
    pub database_folder:PathBuf,
    pub devices:Devices,
    pub access_modes:AccessModes,
    pub configs:ChatConfigurations
}

impl ProxDatabase {
    pub fn from_parts(
        files:Files,
        folders:Folders,
        chats:Chats,
        tags:Tags,
        personal_info:PersonalInformation,
        database_folder:PathBuf,
        devices:Devices,
        access_modes:AccessModes,
        configs:ChatConfigurations
    ) -> Self {
        Self { files, folders, chats, tags, personal_info, database_folder, devices, access_modes, configs }
    }
    pub fn new(pseudonym:String, password_hash:String, database_folder:PathBuf) -> Self {
        if create_or_repair_database_folder_structure(database_folder.clone()) {
            load_from_disk(database_folder.clone()).unwrap()
        }
        else {
            Self { files: Files::new(), folders: Folders::new(), tags: Tags::new(), personal_info: PersonalInformation::new(pseudonym, password_hash), database_folder, chats:Chats::new(), devices:Devices::new(), access_modes:AccessModes::new(), configs:ChatConfigurations::new() }
        }
    }
    pub fn new_just_data(pseudonym:String, password_hash:String) -> ProxDatabase {
        Self { files: Files::new(), folders: Folders::new(), tags: Tags::new(), personal_info: PersonalInformation::new(pseudonym, password_hash), database_folder:PathBuf::from("a/a/a/a/a/a/a/a"), chats:Chats::new(), devices:Devices::new(), access_modes:AccessModes::new(), configs:ChatConfigurations::new() }
    }
    pub fn add_desc_and_tags(&mut self, desc_type:DescriptionTarget, desc:Description, tags:HashSet<TagID>) {
        match desc_type {
            DescriptionTarget::File(id) => self.files.get_file_mut(id).add_desc_tags(desc, tags),
            DescriptionTarget::Folder(id) => self.folders.get_folder_mut(id).add_desc_tags(desc, tags),
        }
    }
    pub fn insert_access_mode(&mut self, access_mode:AccessMode) {
        let id = access_mode.get_id();
        self.access_modes.get_modes_mut().insert(id, access_mode);
        for i in (id + 1)..self.access_modes.get_modes().len() {
            self.access_modes.get_modes_mut()[i].set_id(i);
        }
        for i in 0..self.chats.get_chats().len() {
            let mut new_set = HashSet::with_capacity(16);
            let mut chat = self.chats.get_chats_mut().get_mut(&i).unwrap();
            for access_mode_id in chat.access_modes.iter() {
                if *access_mode_id >= id {
                    new_set.insert(*access_mode_id + 1);
                }
                else {
                    new_set.insert(*access_mode_id);
                }
            }
            match &mut chat.latest_used_config {
                Some(last_config) => {
                    let mut new_set = HashSet::with_capacity(16);
                    for access_mode_id in last_config.access_modes.iter() {
                        if *access_mode_id >= id {
                            new_set.insert(*access_mode_id + 1);
                        }
                        else {
                            new_set.insert(*access_mode_id);
                        }
                    }
                    last_config.access_modes = new_set;
                },
                None => (),
            }
            chat.access_modes = new_set;
        }
        for i in 0..self.files.len() {
            let mut new_set = HashSet::with_capacity(16);
            let mut file = self.files.get_file_mut(i);
            for access_mode_id in file.access_modes.iter() {
                if *access_mode_id >= id {
                    new_set.insert(*access_mode_id + 1);
                }
                else {
                    new_set.insert(*access_mode_id);
                }
            }
            file.access_modes = new_set;
        }
        for i in 0..self.folders.number_of_folders() {
            let mut new_set = HashSet::with_capacity(16);
            let mut folder = self.folders.get_folder_mut(i);
            for access_mode_id in folder.access_modes.iter() {
                if *access_mode_id >= id {
                    new_set.insert(*access_mode_id + 1);
                }
                else {
                    new_set.insert(*access_mode_id);
                }
            }
            folder.access_modes = new_set;
        }
        for i in 0..self.configs.get_configs().len() {
            let mut new_set = HashSet::with_capacity(16);
            let mut config = &mut self.configs.get_configs_mut()[i];
            for access_mode_id in config.access_modes.iter() {
                if *access_mode_id >= id {
                    new_set.insert(*access_mode_id + 1);
                }
                else {
                    new_set.insert(*access_mode_id);
                }
            }
            config.access_modes = new_set;
        }
    }

    pub fn insert_tag(&mut self, tag:Tag) {
        let id = tag.get_id();
        self.tags.get_tags_mut().insert(id, tag);
        for i in (id + 1)..self.tags.get_tags().len() {
            self.tags.get_tags_mut()[i].set_id(i);
        }
        
        for i in 0..self.chats.get_chats().len() {
            let mut new_set = HashSet::with_capacity(16);
            let mut chat = self.chats.get_chats_mut().get_mut(&i).unwrap();
            for tag_id in chat.tags.iter() {
                if *tag_id >= id {
                    new_set.insert(*tag_id + 1);
                }
                else {
                    new_set.insert(*tag_id);
                }
            }
            match &mut chat.latest_used_config {
                Some(last_config) => {
                    let mut new_set = HashSet::with_capacity(16);
                    for tag_id in last_config.tags.iter() {
                        if *tag_id >= id {
                            new_set.insert(*tag_id + 1);
                        }
                        else {
                            new_set.insert(*tag_id);
                        }
                    }
                    last_config.tags = new_set;
                },
                None => (),
            }
            chat.tags = new_set;
        }
        for i in 0..self.files.len() {
            let mut new_set = HashSet::with_capacity(16);
            let mut file = self.files.get_file_mut(i);
            for tag_id in file.tags.iter() {
                if *tag_id >= id {
                    new_set.insert(*tag_id + 1);
                }
                else {
                    new_set.insert(*tag_id);
                }
            }
            file.tags = new_set;
        }
        for i in 0..self.folders.number_of_folders() {
            let mut new_set = HashSet::with_capacity(16);
            let mut folder = self.folders.get_folder_mut(i);
            for tag_id in folder.tags.iter() {
                if *tag_id >= id {
                    new_set.insert(*tag_id + 1);
                }
                else {
                    new_set.insert(*tag_id);
                }
            }
            folder.tags = new_set;
        }
        for i in 0..self.access_modes.get_modes().len() {
            let mut new_set = HashSet::with_capacity(16);
            let mut access_mode = &mut self.access_modes.get_modes_mut()[i];
            for tag_id in access_mode.tags.iter() {
                if *tag_id >= id {
                    new_set.insert(*tag_id + 1);
                }
                else {
                    new_set.insert(*tag_id);
                }
            }
            access_mode.tags = new_set;
        }
        for i in 0..self.configs.get_configs().len() {
            let mut new_set = HashSet::with_capacity(16);
            let mut config = &mut self.configs.get_configs_mut()[i];
            for tag_id in config.tags.iter() {
                if *tag_id >= id {
                    new_set.insert(*tag_id + 1);
                }
                else {
                    new_set.insert(*tag_id);
                }
            }
            config.tags = new_set;
        }
    }
    pub fn insert_chat(&mut self, chat:Chat) {
        let id = chat.get_id();
        for i in (id..self.chats.get_chats().len()).rev() {
            let mut chat = self.chats.get_chats_mut().remove(&i).unwrap();
            chat.id = (i + 1);
            self.chats.get_chats_mut().insert((i + 1), chat);
        }
        self.chats.get_chats_mut().insert(id, chat);
    }
    pub fn insert_file(&mut self, file:ProxFile) {
        let id = file.get_id();
        self.files.insert_file(file);
        
        for i in 0..self.folders.number_of_folders() {
            let folder = self.folders.get_folder_mut(i);
            for file in folder.files.iter_mut() {
                if *file >= id {
                    *file = *file + 1;
                }
            }
        }
    }
    pub fn insert_folder(&mut self, folder:ProxFolder) {
        let id = folder.get_id();
        self.folders.insert_folder(folder);
        
    }
    pub fn insert_device(&mut self, device:Device) {
        let id = device.id;
        self.devices.get_devices_mut().insert(id, device);
        for i in (id + 1)..self.devices.get_devices().len() {
            self.devices.get_devices_mut()[i].id = i;
        }
        
        for i in 0..self.chats.get_chats().len() {
            let mut chat = self.chats.get_chats_mut().get_mut(&i).unwrap();
            if chat.origin_device >= id {
                chat.origin_device += 1;
            }
        }
        for i in 0..self.files.len() {
            let mut file = self.files.get_file_mut(i);
            if file.from_device >= id {
                file.from_device += 1;
            }
        }
        for i in 0..self.folders.number_of_folders() {
            let mut folder = self.folders.get_folder_mut(i);
            if folder.from_device >= id {
                folder.from_device += 1;
            }
        }
    }
    pub fn insert_config(&mut self, config:ChatConfiguration) {
        self.configs.insert_config(config.clone());
        for i in 0..self.chats.get_chats().len() {
            let chat = self.chats.get_chats_mut().get_mut(&i).unwrap();
            match &mut chat.config {
                Some(id) => if *id >= config.id {
                    *id += 1;
                },
                None => (),
            }
            match &mut chat.latest_used_config {
                Some(last_config) => if last_config.id >= config.id {
                    last_config.id += 1;
                },
                None => (),
            }
        }
    }
    pub fn insert_or_update(&mut self, item:DatabaseItem) -> bool { //true if updated, false if inserted
        match item {
            DatabaseItem::AccessMode(access_mode) => {
                if self.access_modes.get_modes()[access_mode.get_id()].added_on == access_mode.added_on {
                    self.access_modes.update_mode(access_mode);
                    true
                }
                else {
                    self.insert_access_mode(access_mode);
                    false
                }
            },
            DatabaseItem::Chat(chat) => {
                let id = chat.get_id();
                if self.chats.get_chats().get(&id).unwrap().start_date == chat.start_date {
                    self.chats.get_chats_mut().insert(id, chat);
                    true
                }
                else {
                    self.insert_chat(chat);
                    false
                }
            },
            DatabaseItem::Device(device) => {
                let id = device.get_id();
                if self.devices.get_devices()[id].added_on == device.added_on {
                    self.devices.get_devices_mut()[id] = device;
                    true
                }
                else {
                    self.insert_device(device);
                    false
                }
            },
            DatabaseItem::File(file) => {
                let id = file.get_id();
                if self.files.get_file_mut(id).added_at == file.added_at {
                    *self.files.get_file_mut(id) = file;
                    true
                }
                else {
                    self.insert_file(file);
                    false
                }
            },
            DatabaseItem::Folder(folder) => {
                let id = folder.get_id();
                if self.folders.get_folder_mut(id).added_at == folder.added_at {
                    *self.folders.get_folder_mut(id) = folder;
                    true
                }  
                else {
                    self.insert_folder(folder);
                    false
                }
            },
            DatabaseItem::Tag(tag) => {
                let id = tag.get_id();
                if self.tags.get_tags_mut()[id].created_at == tag.created_at {
                    self.tags.get_tags_mut()[id] = tag;
                    true
                }
                else {
                    self.insert_tag(tag);
                    false
                }
            },
            DatabaseItem::ChatConfig(config) => {
                let id = config.id;
                if self.configs.get_configs()[id].created_on == config.created_on {
                    self.configs.get_configs_mut()[id] = config;
                    true
                }
                else {
                    self.insert_config(config);
                    false
                }
            }
            DatabaseItem::UserData(user_data) => {
                self.personal_info.user_data = user_data;
                true
            }
        }
    }
    pub fn insert_directly(&mut self, item:DatabaseItem) {
        match item {
            DatabaseItem::AccessMode(access_mode) => {
                    self.insert_access_mode(access_mode);
            },
            DatabaseItem::Chat(chat) => {
                    self.insert_chat(chat);
            },
            DatabaseItem::Device(device) => {
                    self.insert_device(device);
            },
            DatabaseItem::File(file) => {
                    self.insert_file(file);
            },
            DatabaseItem::Folder(folder) => {
                    self.insert_folder(folder);
            },
            DatabaseItem::Tag(tag) => {
                    self.insert_tag(tag);
            },
            DatabaseItem::ChatConfig(config) => {
                self.insert_config(config);
            },
            DatabaseItem::UserData(user_data) => {
                self.personal_info.user_data = user_data;
            }
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub enum DatabaseItem {
    Device(Device),
    Chat(Chat),
    Folder(ProxFolder),
    File(ProxFile),
    Tag(Tag),
    AccessMode(AccessMode),
    UserData(UserData),
    ChatConfig(ChatConfiguration)
}

impl DatabaseItem {
    pub fn get_id(&self) -> DatabaseItemID {
        match self {
            Self::AccessMode(access_mode) => DatabaseItemID::AccessMode(access_mode.get_id()),
            Self::Chat(chat) => DatabaseItemID::Chat(chat.get_id()),
            Self::Device(device) => DatabaseItemID::Device(device.get_id()),
            Self::File(file) => DatabaseItemID::File(file.get_id()),
            Self::Folder(folder) => DatabaseItemID::Folder(folder.get_id()),
            Self::Tag(tag) => DatabaseItemID::Tag(tag.get_id()),
            Self::ChatConfig(config) => DatabaseItemID::ChatConfiguration(config.id),
            Self::UserData(user_data) => DatabaseItemID::UserData
        }
    }
    
    pub fn set_id(&mut self, new_id:DatabaseItemID) {
        match self {
            Self::AccessMode(access_mode) => match new_id {
                DatabaseItemID::AccessMode(id) => access_mode.set_id(id),
                _ => panic!("Wrong kind of ID")
            },
            Self::Chat(chat) => match new_id {
                DatabaseItemID::Chat(id) => chat.id = id,
                _ => panic!("wrong kind of ID")
            },
            Self::Device(device) => match new_id {
                DatabaseItemID::Device(id) => device.id = id,
                _ => panic!("wrong kind of ID")
            },
            Self::File(file) => match new_id {
                DatabaseItemID::File(id) => file.id = id,
                _ => panic!("wrong kind of ID")
            },
            Self::Folder(folder) => match new_id {
                DatabaseItemID::Folder(id) => folder.id = id,
                _ => panic!("wrong kind of ID")
            },
            Self::Tag(tag) => match new_id {
                DatabaseItemID::Tag(id) => tag.set_id(id),
                _ => panic!("wrong kind of ID")
            },
            Self::ChatConfig(config) => match new_id {
                DatabaseItemID::ChatConfiguration(id) => config.id = id,
                _ => panic!("wrong kind of ID")
            }
            Self::UserData(user_data) => ()
        }
    }
}


#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum DatabaseItemID {
    Device(DeviceID),
    Chat(ChatID),
    Folder(FolderID),
    File(FileID),
    Tag(TagID),
    AccessMode(AccessModeID),
    UserData,
    ChatConfiguration(ChatConfigID)
}

impl Step for DatabaseItemID {
    fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
        let start_id = match start {
            DatabaseItemID::AccessMode(id) => *id,
            DatabaseItemID::Chat(id) => *id,
            DatabaseItemID::Device(id) => *id,
            DatabaseItemID::File(id) => *id,
            DatabaseItemID::Folder(id) => *id,
            DatabaseItemID::Tag(id) => *id,
            DatabaseItemID::ChatConfiguration(id) => *id,
            DatabaseItemID::UserData => 1
        };
        let end_id = match end {
            DatabaseItemID::AccessMode(id) => *id,
            DatabaseItemID::Chat(id) => *id,
            DatabaseItemID::Device(id) => *id,
            DatabaseItemID::File(id) => *id,
            DatabaseItemID::Folder(id) => *id,
            DatabaseItemID::Tag(id) => *id,
            DatabaseItemID::ChatConfiguration(id) => *id,
            DatabaseItemID::UserData => 0
        };
        if start_id > end_id {
            (usize::MAX, None)
        }
        else {
            (end_id - start_id, Some(end_id - start_id))
        }
    }
    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        match start {
            DatabaseItemID::AccessMode(id) => Some(DatabaseItemID::AccessMode(id + 1)),
            DatabaseItemID::Chat(id) => Some(DatabaseItemID::Chat(id + 1)),
            DatabaseItemID::File(id) => Some(DatabaseItemID::File(id + 1)),
            DatabaseItemID::Folder(id) => Some(DatabaseItemID::Folder(id + 1)),
            DatabaseItemID::Device(id) => Some(DatabaseItemID::Device(id + 1)),
            DatabaseItemID::Tag(id) => Some(DatabaseItemID::Tag(id + 1)),
            DatabaseItemID::ChatConfiguration(id) => Some(DatabaseItemID::ChatConfiguration(id + 1)),
            DatabaseItemID::UserData => None,
        }
    }
    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        match start {
            DatabaseItemID::AccessMode(id) => Some(DatabaseItemID::AccessMode(id - 1)),
            DatabaseItemID::Chat(id) => Some(DatabaseItemID::Chat(id - 1)),
            DatabaseItemID::File(id) => Some(DatabaseItemID::File(id - 1)),
            DatabaseItemID::Folder(id) => Some(DatabaseItemID::Folder(id - 1)),
            DatabaseItemID::Device(id) => Some(DatabaseItemID::Device(id - 1)),
            DatabaseItemID::Tag(id) => Some(DatabaseItemID::Tag(id - 1)),
            DatabaseItemID::ChatConfiguration(id) => Some(DatabaseItemID::ChatConfiguration(id - 1)),
            DatabaseItemID::UserData => None,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub enum DatabaseInfoRequest {
    NumbersOfItems,
    LatestItems,
    UnknownUpdates {access_key:String},
}

#[derive(Clone, Serialize, Deserialize)]
pub enum DatabaseRequestVariant {
    GetAll,
    Get(DatabaseItemID),
    Update(DatabaseItem),
    Info(DatabaseInfoRequest),
    Add(DatabaseItem),
    NewAuthKey,
    VerifyAuthKey(String),
    GetAgentPrompt(AgentPrompt), // >:(
    Save
}

pub struct DatabaseRequest {
    response_sender:Sender<DatabaseReply>,
    variant:DatabaseRequestVariant,
    auth_key:Option<String>,
}

impl DatabaseRequest {
    pub fn new(variant:DatabaseRequestVariant, auth_key:Option<String>) -> (Self, Receiver<DatabaseReply>) {
        let (response_sender, response_receiver) = channel();
        (
            Self {
                variant,
                response_sender,
                auth_key
            },
            response_receiver
        )
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub enum DatabaseInfoReply {
    NumbersOfItems {devices:usize, chats:usize, folders:usize, files:usize, tags:usize, access_modes:usize},
    LatestItems {items:Vec<Option<DatabaseItem>>},
    UnknownUpdates {updates:Vec<(DatabaseItemID, DatabaseItem)>},
}

#[derive(Clone, Serialize, Deserialize)]
pub enum DatabaseReplyVariant {
    RequestExecuted,
    AddedItem(DatabaseItemID),
    ReturnedItem(DatabaseItem),
    CorrectAuth,
    WrongAuth,
    NewAuth(String),
    Info(DatabaseInfoReply),
    ConstructedPrompt(WholeContext),
    ReplyAll(ProxDatabase),
    Saved,
    Error(DatabaseError)
}


#[derive(Clone, Serialize, Deserialize)]
pub enum DatabaseError {
    SavingError
}

pub struct DatabaseReply {
    pub variant:DatabaseReplyVariant
}

pub struct ClientSessionData {
    pending_updates:VecDeque<(DatabaseItemID, DatabaseItem)>,
}

pub struct DatabaseHandler {
    priority_request_rcv:Receiver<DatabaseRequest>,
    request_rcv:Receiver<DatabaseRequest>,
    database:ProxDatabase,
    auth_sessions:HashMap<String, ClientSessionData>,
    auth_sessions_rng:StdRng,
    changed_since_last_save:bool
}


impl DatabaseHandler {
    pub fn new(priority_request_rcv:Receiver<DatabaseRequest>, request_rcv:Receiver<DatabaseRequest>, database:ProxDatabase) -> Self {
        Self { priority_request_rcv, request_rcv, database, auth_sessions:HashMap::with_capacity(32), auth_sessions_rng:StdRng::from_os_rng(), changed_since_last_save:true }
    }
    pub fn handling_loop(&mut self) {
        loop {
            match self.priority_request_rcv.recv_timeout(Duration::from_millis(30_000)) {
                Ok(request) => {
                    self.handle_request(request);
                },
                Err(error) => match error {
                    RecvTimeoutError::Timeout => (),
                    RecvTimeoutError::Disconnected => panic!("Error accessing the database, tunnel closed"),
                }
            }
            loop {
                if self.priority_request_rcv.is_empty() {
                    if let Ok(request) = self.request_rcv.try_recv() {
                        self.handle_request(request);
                    }
                    else {
                        break;
                    }
                }
                else {
                    break;
                }
            }
            
        }
    }
    fn handle_get_request(&self, id:DatabaseItemID, response_sender:Sender<DatabaseReply>) -> Result<(), SendError<DatabaseReply>> {
        match id {
            DatabaseItemID::Tag(tagid) => response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::Tag(self.database.tags.get_tags()[tagid].clone()))}),
            DatabaseItemID::AccessMode(modeid) => response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::AccessMode(self.database.access_modes.get_modes()[modeid].clone()))}),
            DatabaseItemID::Device(deviceid) => response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::Device(self.database.devices.get_devices()[deviceid].clone()))}),
            DatabaseItemID::Chat(chatid) => response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::Chat(self.database.chats.get_chats().get(&chatid).unwrap().clone()))}),
            DatabaseItemID::File(fileid) => response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::File(self.database.files.get_file_by_id(fileid).clone()))}),
            DatabaseItemID::Folder(folderid) => response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::Folder(self.database.folders.get_folder_by_id(folderid).clone()))}),
            DatabaseItemID::ChatConfiguration(configid) => response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::ChatConfig(self.database.configs.get_configs()[configid].clone()))}),
            DatabaseItemID::UserData => response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::UserData(self.database.personal_info.user_data.clone()))}),
        }
    }
    fn handle_update_request(&mut self, item:DatabaseItem, response_sender:Sender<DatabaseReply>, auth_key:Option<String>) -> Result<(), SendError<DatabaseReply>> {
        match auth_key {
            Some(key) => for (user, data) in self.auth_sessions.iter_mut() {
                if user != &key {
                    data.pending_updates.push_back((item.get_id(), item.clone()));
                }
            },
            None => for (user, data) in self.auth_sessions.iter_mut() {
                data.pending_updates.push_back((item.get_id(), item.clone()));
            }
        }
        self.changed_since_last_save = true;
        match item {
            DatabaseItem::Tag(tag) => {self.database.tags.update_tag(tag); response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })},
            DatabaseItem::AccessMode(access_mode) => {self.database.access_modes.update_mode(access_mode); response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })},
            DatabaseItem::Device(device) => {self.database.devices.update_device(device); response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })},
            DatabaseItem::Chat(chat) => {self.database.chats.update_chat(chat); response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })},
            DatabaseItem::File(file) => {let id = file.get_id(); *self.database.files.get_file_mut(id) = file; response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })},
            DatabaseItem::Folder(folder) => {let id = folder.get_id();*self.database.folders.get_folder_mut(id) = folder; response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })},
            DatabaseItem::ChatConfig(config) => {self.database.configs.update_config(config); response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })}
            DatabaseItem::UserData(user_data) => {self.database.personal_info.user_data = user_data; response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })}
        }  
    }
    fn handle_add_request(&mut self, item:DatabaseItem, response_sender:Sender<DatabaseReply>, auth_key:Option<String>) -> Result<(), SendError<DatabaseReply>> {
        self.changed_since_last_save = true;
        let s_item = item.clone();
        let (res, id) = match item {
            DatabaseItem::Tag(tag) => {let id = self.database.tags.add_tag_raw(tag); (response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::Tag(id)) }), DatabaseItemID::Tag(id))},
            DatabaseItem::AccessMode(access_mode) => {let id = self.database.access_modes.add_mode(access_mode); (response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::AccessMode(id)) }), DatabaseItemID::AccessMode(id))},
            DatabaseItem::Device(device) => {let id = self.database.devices.add_device(device); (response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::Device(id)) }), DatabaseItemID::Device(id))},
            DatabaseItem::Chat(chat) => {let id = self.database.chats.add_chat_raw(chat); (response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::Chat(id)) }), DatabaseItemID::Chat(id))},
            DatabaseItem::File(file) => {let id = self.database.files.add_file_raw(file); (response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::File(id)) }), DatabaseItemID::File(id))},
            DatabaseItem::Folder(folder) => {let id = self.database.folders.add_folder_raw(folder); (response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::Folder(id)) }), DatabaseItemID::Folder(id))},
            DatabaseItem::ChatConfig(config) => {let id = self.database.configs.add_config(config); (response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::ChatConfiguration(id)) }), DatabaseItemID::ChatConfiguration(id))},
            DatabaseItem::UserData(user_data) => {self.database.personal_info.user_data = user_data; (response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::UserData) }), DatabaseItemID::UserData)}
        };
        match auth_key {
            Some(key) => for (user, data) in self.auth_sessions.iter_mut() {
                if user != &key {
                    data.pending_updates.push_back((id.clone(), s_item.clone()));
                }
            },
            None => for (user, data) in self.auth_sessions.iter_mut() {
                data.pending_updates.push_back((id.clone(), s_item.clone()));
            }
        }
        res
    }
    fn handle_new_auth_key(&mut self, response_sender:Sender<DatabaseReply>) -> Result<(), SendError<DatabaseReply>> {
        let new_auth = self.auth_sessions_rng.next_u64().to_string();
        self.auth_sessions.insert(new_auth.clone(), ClientSessionData { pending_updates: VecDeque::with_capacity(128) });
        response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::NewAuth(new_auth)})
    }
    fn handle_auth_verification(&mut self, auth:String, response_sender:Sender<DatabaseReply>) -> Result<(), SendError<DatabaseReply>> {
        if self.auth_sessions.contains_key(&auth) {
            response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::CorrectAuth})
        }
        else { 
            response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::WrongAuth})
        }
    }
    fn handle_info_request(&mut self, info_request:DatabaseInfoRequest, response_sender:Sender<DatabaseReply>) -> Result<(), SendError<DatabaseReply>> {
        match info_request {
            DatabaseInfoRequest::NumbersOfItems => {
                response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::Info(
                DatabaseInfoReply::NumbersOfItems
                    { 
                        devices: self.database.devices.get_devices().len(),
                        chats: self.database.chats.get_chats().len(),
                        folders: self.database.folders.number_of_folders(),
                        files: self.database.files.len(),
                        tags: self.database.tags.get_tags().len(),
                        access_modes: self.database.access_modes.get_modes().len()
                    }
                ) })
            },
            DatabaseInfoRequest::LatestItems => {
                response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::Info(
                DatabaseInfoReply::LatestItems { items: vec![
                    self.database.devices.get_devices().last().map(|item| {DatabaseItem::Device(item.clone())}),
                    self.database.access_modes.get_modes().last().map(|item| {DatabaseItem::AccessMode(item.clone())}),
                    self.database.chats.get_last_chat().map(|item| {DatabaseItem::Chat(item.clone())}),
                    self.database.folders.get_last_folder().map(|item| {DatabaseItem::Folder(item.clone())}),
                    self.database.files.get_last_file().map(|item| {DatabaseItem::File(item.clone())}),
                    self.database.tags.get_tags().last().map(|item| {DatabaseItem::Tag(item.clone())}),
                    Some(DatabaseItem::UserData(self.database.personal_info.user_data.clone())),

                ] }
                ) })
            },
            DatabaseInfoRequest::UnknownUpdates { access_key } => {
                response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::Info(
                DatabaseInfoReply::UnknownUpdates { updates: self.auth_sessions.get_mut(&access_key).unwrap().pending_updates.drain(..).collect() }
                ) })
            }
        }
    }
    fn handle_agent_prompt(&self, agent_prompt:AgentPrompt, response_sender:Sender<DatabaseReply>) -> Result<(), SendError<DatabaseReply>> {
        response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::ConstructedPrompt(get_agent_prompt_context(&self.database, agent_prompt))})
    }
    fn handle_getall(&self, response_sender:Sender<DatabaseReply>) -> Result<(), SendError<DatabaseReply>> {
        response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::ReplyAll(self.database.clone()) })
    }
    fn handle_save(&mut self, response_sender:Sender<DatabaseReply>) -> Result<(), SendError<DatabaseReply>> {
        
        if self.changed_since_last_save {
            let db_clone = self.database.clone();
            let dir_path = self.database.database_folder.clone();
            thread::spawn(move || {
                match save_to_disk(db_clone, dir_path) {
                    Ok(saved) => response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::Saved }),
                    Err(error) => response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::SavingError) }), 
                }
            });
            self.changed_since_last_save = false;
            Ok(())
        }
        else {
            self.changed_since_last_save = false;
            response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::Saved })
        }

        
    }
    fn handle_request(&mut self, request:DatabaseRequest) -> Result<(), SendError<DatabaseReply>> {
        match request.variant {
            DatabaseRequestVariant::Get(id) => self.handle_get_request(id, request.response_sender),
            DatabaseRequestVariant::Add(item) => self.handle_add_request(item, request.response_sender, request.auth_key),
            DatabaseRequestVariant::Update(item) => self.handle_update_request(item, request.response_sender, request.auth_key),
            DatabaseRequestVariant::NewAuthKey => self.handle_new_auth_key(request.response_sender),
            DatabaseRequestVariant::VerifyAuthKey(auth) => self.handle_auth_verification(auth, request.response_sender),
            DatabaseRequestVariant::Info(info_request) => self.handle_info_request(info_request, request.response_sender),
            DatabaseRequestVariant::GetAgentPrompt(agent_prompt) => self.handle_agent_prompt(agent_prompt, request.response_sender),
            DatabaseRequestVariant::GetAll => self.handle_getall(request.response_sender),
            DatabaseRequestVariant::Save => self.handle_save(request.response_sender)

        }
    }
}
#[derive(Clone)]
pub struct DatabaseSender {
    prio_queue:Sender<DatabaseRequest>,
    normal_queue:Sender<DatabaseRequest>
}

impl DatabaseSender {
    pub fn send_normal(&self, req:DatabaseRequest) {
        self.normal_queue.send(req);
    }
    pub fn send_prio(&self, req:DatabaseRequest) {
        self.prio_queue.send(req);
    }
}

pub fn launch_database_thread(database:ProxDatabase) -> DatabaseSender {
    let (prio_send, prio_rcv) = channel();
    let (normal_send, normal_rcv) = channel();
    thread::spawn(move || {
        DatabaseHandler::new(prio_rcv, normal_rcv, database).handling_loop();
    });
    DatabaseSender { prio_queue:prio_send, normal_queue:normal_send }
}

pub fn launch_saving_thread(sender:DatabaseSender, timer:Duration) {
    thread::spawn(move || {
        loop {
            thread::sleep(timer.clone());
            let (request, receiver) = DatabaseRequest::new(DatabaseRequestVariant::Save, None);
            sender.send_normal(request);
            match receiver.recv() {
                Ok(_) => (),
                Err(_) => break
            }
        }
    });
}