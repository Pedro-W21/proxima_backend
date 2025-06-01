use std::{collections::{HashMap, HashSet}, path::PathBuf, sync::{mpmc::{channel, Receiver, Sender}, mpsc::SendError}, thread};

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
}

impl ProxDatabase {
    pub fn new(pseudonym:String, password_hash:String, database_folder:PathBuf) -> Self {
        if create_or_repair_database_folder_structure(database_folder.clone()) {

        }
        Self { files: Files::new(), folders: Folders::new(), tags: Tags::new(), personal_info: PersonalInformation::new(pseudonym, password_hash), database_folder, chats:Chats::new(), devices:Devices::new(), access_modes:AccessModes::new() }
    }
    pub fn add_desc_and_tags(&mut self, desc_type:DescriptionTarget, desc:Description, tags:Vec<TagID>) {
        match desc_type {
            DescriptionTarget::File(id) => self.files.get_file_mut(id).add_desc_tags(desc, tags),
            DescriptionTarget::Folder(id) => self.folders.get_folder_mut(id).add_desc_tags(desc, tags),
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
    UserData(UserData)
}
#[derive(Clone, Serialize, Deserialize)]
pub enum DatabaseItemID {
    Device(DeviceID),
    Chat(ChatID),
    Folder(FolderID),
    File(FileID),
    Tag(TagID),
    AccessMode(AccessModeID),
    UserData
}
#[derive(Clone, Serialize, Deserialize)]
pub enum DatabaseInfoRequest {
    NumbersOfItems,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum DatabaseRequestVariant {
    Get(DatabaseItemID),
    Update(DatabaseItem),
    Info(DatabaseInfoRequest),
    Add(DatabaseItem),
    NewAuthKey,
    VerifyAuthKey(String)
}

pub struct DatabaseRequest {
    response_sender:Sender<DatabaseReply>,
    variant:DatabaseRequestVariant,
}

impl DatabaseRequest {
    pub fn new(variant:DatabaseRequestVariant) -> (Self, Receiver<DatabaseReply>) {
        let (response_sender, response_receiver) = channel();
        (
            Self {
                variant,
                response_sender
            },
            response_receiver
        )
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub enum DatabaseInfoReply {
    NumbersOfItems {devices:usize, chats:usize, folders:usize, files:usize, tags:usize, access_modes:usize}
}

#[derive(Clone, Serialize, Deserialize)]
pub enum DatabaseReplyVariant {
    RequestExecuted,
    ReturnedItem(DatabaseItem),
    CorrectAuth,
    WrongAuth,
    NewAuth(String),
    Info(DatabaseInfoReply)
}

pub struct DatabaseReply {
    pub variant:DatabaseReplyVariant
}

pub struct DatabaseHandler {
    priority_request_rcv:Receiver<DatabaseRequest>,
    request_rcv:Receiver<DatabaseRequest>,
    database:ProxDatabase,
    auth_sessions:HashSet<String>,
    auth_sessions_rng:StdRng
}


impl DatabaseHandler {
    pub fn new(priority_request_rcv:Receiver<DatabaseRequest>, request_rcv:Receiver<DatabaseRequest>, database:ProxDatabase) -> Self {
        // FIX THE RNG BEFORE ANY SECURITY GUARANTEES
        Self { priority_request_rcv, request_rcv, database, auth_sessions:HashSet::with_capacity(32), auth_sessions_rng:StdRng::from_seed([200 ; 32]) }
    }
    pub fn handling_loop(&mut self) {
        loop {
            match self.priority_request_rcv.recv() {
                Ok(request) => {
                    self.handle_request(request);
                },
                Err(error) => panic!("Database access error : {}", error)
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
            DatabaseItemID::UserData => response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::UserData(self.database.personal_info.user_data.clone()))}),
        }
    }
    fn handle_update_request(&mut self, item:DatabaseItem, response_sender:Sender<DatabaseReply>) -> Result<(), SendError<DatabaseReply>> {
        match item {
            DatabaseItem::Tag(tag) => {self.database.tags.update_tag(tag); response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })},
            DatabaseItem::AccessMode(access_mode) => {self.database.access_modes.update_mode(access_mode); response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })},
            DatabaseItem::Device(device) => {self.database.devices.update_device(device); response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })},
            DatabaseItem::Chat(chat) => {self.database.chats.update_chat(chat); response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })},
            DatabaseItem::File(file) => {let id = file.get_id(); *self.database.files.get_file_mut(id) = file; response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })},
            DatabaseItem::Folder(folder) => {let id = folder.get_id();*self.database.folders.get_folder_mut(id) = folder; response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })},
            DatabaseItem::UserData(user_data) => {self.database.personal_info.user_data = user_data; response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })}
        }  
    }
    fn handle_add_request(&mut self, item:DatabaseItem, response_sender:Sender<DatabaseReply>) -> Result<(), SendError<DatabaseReply>> {
        match item {
            DatabaseItem::Tag(tag) => {self.database.tags.add_tag_raw(tag); response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })},
            DatabaseItem::AccessMode(access_mode) => {self.database.access_modes.add_mode(access_mode); response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })},
            DatabaseItem::Device(device) => {self.database.devices.add_device(device); response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })},
            DatabaseItem::Chat(chat) => {self.database.chats.add_chat_raw(chat); response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })},
            DatabaseItem::File(file) => {self.database.files.add_file_raw(file); response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })},
            DatabaseItem::Folder(folder) => {self.database.folders.add_folder_raw(folder); response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })},
            DatabaseItem::UserData(user_data) => {self.database.personal_info.user_data = user_data; response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted })}
        }
    }
    fn handle_new_auth_key(&mut self, response_sender:Sender<DatabaseReply>) -> Result<(), SendError<DatabaseReply>> {
        let new_auth = self.auth_sessions_rng.next_u64().to_string();
        self.auth_sessions.insert(new_auth.clone());
        response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::NewAuth(new_auth)})
    }
    fn handle_auth_verification(&mut self, auth:String, response_sender:Sender<DatabaseReply>) -> Result<(), SendError<DatabaseReply>> {
        if self.auth_sessions.contains(&auth) {
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
            }
        }
    }
    fn handle_request(&mut self, request:DatabaseRequest) -> Result<(), SendError<DatabaseReply>> {
        match request.variant {
            DatabaseRequestVariant::Get(id) => self.handle_get_request(id, request.response_sender),
            DatabaseRequestVariant::Add(item) => self.handle_add_request(item, request.response_sender),
            DatabaseRequestVariant::Update(item) => self.handle_update_request(item, request.response_sender),
            DatabaseRequestVariant::NewAuthKey => self.handle_new_auth_key(request.response_sender),
            DatabaseRequestVariant::VerifyAuthKey(auth) => self.handle_auth_verification(auth, request.response_sender),
            DatabaseRequestVariant::Info(info_request) => self.handle_info_request(info_request, request.response_sender)
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