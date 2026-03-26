use std::{collections::{HashMap, HashSet, VecDeque}, iter::Step, path::PathBuf, sync::{LazyLock, mpmc::{Receiver, Sender, channel}, mpsc::{RecvTimeoutError, SendError}}, thread, time::Duration};

use access_modes::{AccessMode, AccessModeID, AccessModes};
use chats::{Chat, ChatID, Chats};
use chrono::{DateTime, TimeDelta, Utc};
use description::{Description, DescriptionTarget};
use devices::{Device, DeviceID, Devices};
use files::{FileID, Files, ProxFile};
use folders::{FolderID, Folders, ProxFolder};
use loading_saving::create_or_repair_database_folder_structure;
use rand::{rngs::StdRng, RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use tags::{Tag, TagID, Tags};
use user::{PersonalInformation, UserData};

use crate::{ai_interaction::create_prompt::AgentPrompt, database::{access_modes::AMSetting, configuration::{ChatConfigID, ChatConfiguration, ChatConfigurations}, context::WholeContext, jobs::{Job, JobID, Jobs}, loading_saving::{load_from_disk, save_to_disk}, media::{Base64EncodedString, Media, MediaHash, MediaStorage}, memories::{MemReqMax, Memories, Memory, MemoryID, MemoryRequest}, notifications::{Notification, NotificationID, Notifications}, user::UserStats}};

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
pub mod media;
pub mod memories;
pub mod notifications;
pub mod jobs;

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
    pub configs:ChatConfigurations,
    pub media:MediaStorage,
    pub memories:Memories,
    pub notifications:Notifications,
    pub jobs:Jobs,
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
        configs:ChatConfigurations,
        media:MediaStorage,
        memories:Memories,
        notifications:Notifications,
        jobs:Jobs
    ) -> Self {
        Self { files, folders, chats, tags, personal_info, database_folder, devices, access_modes, configs, media, memories, notifications, jobs }
    }
    pub fn new(pseudonym:String, password_hash:String, database_folder:PathBuf) -> Self {
        if create_or_repair_database_folder_structure(database_folder.clone()) {
            let mut data = load_from_disk(database_folder.clone()).unwrap();
            data.personal_info.user_data.pseudonym = pseudonym;
            data.personal_info.user_data.password_hash = password_hash;
            data
        }
        else {
            Self { files: Files::new(), folders: Folders::new(), tags: Tags::new(), personal_info: PersonalInformation::new(pseudonym, password_hash), database_folder, chats:Chats::new(), devices:Devices::new(), access_modes:AccessModes::new(), configs:ChatConfigurations::new(), media:MediaStorage::new(), memories:Memories::new(), notifications:Notifications::new(), jobs:Jobs::new() }
        }
    }
    pub fn new_just_data(pseudonym:String, password_hash:String) -> ProxDatabase {
        Self { files: Files::new(), folders: Folders::new(), tags: Tags::new(), personal_info: PersonalInformation::new(pseudonym, password_hash), database_folder:PathBuf::from("a/a/a/a/a/a/a/a"), chats:Chats::new(), devices:Devices::new(), access_modes:AccessModes::new(), configs:ChatConfigurations::new(), media:MediaStorage::new(), memories:Memories::new(), notifications:Notifications::new(), jobs:Jobs::new() }
    }
    pub fn get_request(&self, id:DatabaseItemID) -> DatabaseReply {
        match id.clone() {
            DatabaseItemID::Tag(tagid) => if let Some(tag) = self.tags.get_tags().get(&tagid) {
                DatabaseReply {variant : DatabaseReplyVariant::ReturnedItem(DatabaseItem::Tag(tag.clone()))}
            }
            else {
                DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotFound(id)) }
            },
            DatabaseItemID::AccessMode(modeid) => if let Some(access_mode) = self.access_modes.get_modes().get(&modeid) {
                DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::AccessMode(access_mode.clone()))}
            }
            else {
                DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotFound(id)) }
            }
            DatabaseItemID::Device(deviceid) => if let Some(device) = self.devices.get_devices().get(&deviceid) {
                DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::Device(device.clone()))}
            }
            else {
                DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotFound(id)) }
            }
            DatabaseItemID::Chat(chatid) => if let Some(chat) = self.chats.get_chats().get(&chatid) {
                DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::Chat(chat.clone()))}
            }
            else {
                DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotFound(id)) }
            }
            DatabaseItemID::File(fileid) => if let Some(file) = self.files.get_file_by_id(fileid) {
                DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::File(file.clone()))}
            }
            else {
                DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotFound(id)) }
            },
            DatabaseItemID::Folder(folderid) => if let Some(folder) = self.folders.get_folder_by_id(folderid) {
                DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::Folder(folder.clone()))}
            }
            else {
                DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotFound(id)) }
            },
            DatabaseItemID::ChatConfiguration(configid) => if let Some(config) = self.configs.get_configs().get(&configid) {
                DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::ChatConfig(config.clone()))}
            }
            else {
                DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotFound(id)) }
            },
            DatabaseItemID::UserData => DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::UserData(self.personal_info.user_data.clone()))},
            DatabaseItemID::Media(mediaid) => if let Some((media, data)) = self.media.get_media_with_data(&mediaid, self.database_folder.clone()) {
                DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::Media(media.clone(), Base64EncodedString::new(data)))}
            }
            else {
                DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotFound(id)) }
            },
            DatabaseItemID::Memory(memoryid) => if let Some((memory, data)) = self.memories.get_memory_with_data(memoryid, self.database_folder.clone()) {
                DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::Memory(memory.clone(), data))}
            }
            else {
                DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotFound(id)) }
            },
            DatabaseItemID::Notification(notif) => if let Some(notification) = self.notifications.get_notifications().get(&notif) {
                DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::Notification(notification.clone()))}
            }
            else {
                DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotFound(id)) }
            },
            DatabaseItemID::UserStats => DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::UserStats(self.personal_info.user_stats.clone()))},
            DatabaseItemID::Job(job_id) => if let Some(job) = self.jobs.get_job(job_id) {
                DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::Job(job.clone()))}
            }
            else {
                DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotFound(id)) }
            }
        }
    }
    pub fn update_request(&mut self, item:DatabaseItem) -> DatabaseReply {
        
        match item {
            DatabaseItem::Tag(tag) if self.tags.update_tag(tag.clone()) => DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted },
            DatabaseItem::AccessMode(access_mode) if self.access_modes.update_mode(access_mode.clone()) => {DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted }},
            DatabaseItem::Device(device) if self.devices.update_device(device.clone()) => {DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted }},
            DatabaseItem::Chat(chat) if self.chats.update_chat(chat.clone()) => {DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted }},
            DatabaseItem::File(file) => {let id = file.get_id(); if self.files.get_file_mut(id).and_then(|f| {
                *f = file; Some(0_u8)
            }).is_some() {
                DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted }
            } else {
                DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotFound(DatabaseItemID::File(id))) }
            }},
            DatabaseItem::Folder(folder) => {let id = folder.get_id();if self.folders.get_folder_mut(id).and_then(|f| {
                *f = folder; Some(0_u8)
            }).is_some() {
                DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted }
            } else {
                DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotFound(DatabaseItemID::File(id))) }
            }},
            DatabaseItem::ChatConfig(config) if self.configs.update_config(config.clone()) => {DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted }},
            DatabaseItem::Media(media, data) if self.media.update_media(media.clone(), data.get_data(), self.database_folder.clone()) => {DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted }},
            DatabaseItem::Memory(memory, data) if self.memories.update_memory(memory.id, data.clone(), self.database_folder.clone()) => {DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted }},
            DatabaseItem::Notification(notif) if self.notifications.insert_notification_raw(notif.clone()) => {DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted }},
            DatabaseItem::UserData(user_data) => {self.personal_info.user_data = user_data; DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted }},
            DatabaseItem::UserStats(user_stats) => {self.personal_info.user_stats = user_stats; DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted }},
            DatabaseItem::Job(job) if self.jobs.update_job(job.clone()) => {DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted }},
            _ => DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotFound(item.get_id())) }
        }  
    }
    pub fn add_request(&mut self, item:DatabaseItem) -> (DatabaseReply, DatabaseItemID) {
        match item {
            DatabaseItem::Tag(tag) => {let id = self.tags.add_tag_raw(tag); (DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::Tag(id)) }, DatabaseItemID::Tag(id))},
            DatabaseItem::AccessMode(access_mode) => {let id = self.access_modes.add_mode(access_mode); (DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::AccessMode(id)) }, DatabaseItemID::AccessMode(id))},
            DatabaseItem::Device(device) => {let id = self.devices.add_device(device); (DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::Device(id)) }, DatabaseItemID::Device(id))},
            DatabaseItem::Chat(chat) => {let id = self.chats.add_chat_raw(chat); (DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::Chat(id)) }, DatabaseItemID::Chat(id))},
            DatabaseItem::File(file) => {let id = self.files.add_file_raw(file); (DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::File(id)) }, DatabaseItemID::File(id))},
            DatabaseItem::Folder(folder) => {let id = self.folders.add_folder_raw(folder); (DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::Folder(id)) }, DatabaseItemID::Folder(id))},
            DatabaseItem::ChatConfig(config) => {let id = self.configs.add_config(config); (DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::ChatConfiguration(id)) }, DatabaseItemID::ChatConfiguration(id))},
            DatabaseItem::Media(media, data) => {let id = self.media.add_media(data.get_data(), media.tags, media.access_modes, media.file_name, self.database_folder.clone(), media.media_type); (DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::Media(id.clone())) }, DatabaseItemID::Media(id))},
            DatabaseItem::Memory(memory, data) => {let id = self.memories.add_memory(data, memory.access_modes, memory.tags, self.database_folder.clone(), memory.kind.clone()); (DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::Memory(id)) }, DatabaseItemID::Memory(id))},
            DatabaseItem::UserData(user_data) => {self.personal_info.user_data = user_data; (DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::UserData) }, DatabaseItemID::UserData)},
            DatabaseItem::UserStats(user_stats) => {self.personal_info.user_stats = user_stats; (DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::UserStats) }, DatabaseItemID::UserStats)},
            DatabaseItem::Notification(notif) => {let id = self.notifications.add_notification(notif); (DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::Notification(id)) }, DatabaseItemID::Notification(id))},
            DatabaseItem::Job(job) => {let id = self.jobs.add_job(job); (DatabaseReply { variant: DatabaseReplyVariant::AddedItem(DatabaseItemID::Job(id)) }, DatabaseItemID::Job(id))}
        }
    }
    pub fn remove_request(&mut self, id:DatabaseItemID) -> DatabaseReply {
        match id {
            DatabaseItemID::Notification(notif) => {
                self.notifications.remove_notification(notif);
            },
            DatabaseItemID::Job(job) => {
                println!("[database] removing job {job}");
                self.jobs.remove_job(job);
            },
            DatabaseItemID::Chat(chat) => {
                println!("[database] removing chat {chat}");
                self.chats.remove_chat(chat);
            },
            _ => return DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotDeletable(id)) }
        }
        DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted }
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
    UserStats(UserStats),
    Job(Job),
    ChatConfig(ChatConfiguration),
    Media(Media, Base64EncodedString),
    Memory(Memory, String),
    Notification(Notification)
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
            Self::UserData(user_data) => DatabaseItemID::UserData,
            Self::UserStats(user_stats) => DatabaseItemID::UserStats,
            Self::Media(media, _) => DatabaseItemID::Media(media.hash.clone()),
            Self::Memory(memory, _) => DatabaseItemID::Memory(memory.id),
            Self::Notification(notif) => DatabaseItemID::Notification(notif.id),
            Self::Job(job) => DatabaseItemID::Job(job.id)
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
            },
            Self::Media(med, _) => match new_id {
                DatabaseItemID::Media(id) => med.hash = id,
                _ => panic!("wrong kind of ID")
            },
            Self::Memory(memory, _) => match new_id {
                DatabaseItemID::Memory(id) => memory.id = id,
                _ => panic!("wrong kind of ID")
            },
            Self::Notification(notif) => match new_id {
                DatabaseItemID::Notification(id) => notif.id = id,
                _ => panic!("wrong kind of ID")
            },
            Self::Job(job) => match new_id {
                DatabaseItemID::Job(id) => job.id = id,
                _ => panic!("wrong kind of ID")
            },
            Self::UserData(user_data) => (),
            Self::UserStats(user_stats) => ()
        }
    }
}


#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum DatabaseItemID {
    Device(DeviceID),
    Chat(ChatID),
    Folder(FolderID),
    File(FileID),
    Tag(TagID),
    AccessMode(AccessModeID),
    UserData,
    UserStats,
    ChatConfiguration(ChatConfigID),
    Media(MediaHash),
    Memory(MemoryID),
    Notification(NotificationID),
    Job(JobID)
}

impl DatabaseItemID {
    pub fn is_media(&self) -> bool {
        match self {
            Self::Media(_) => true,
            _ => false
        }
    }
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
            DatabaseItemID::Media(media) => 1,
            DatabaseItemID::Memory(id) => *id,
            DatabaseItemID::Notification(id) => *id,
            DatabaseItemID::Job(id) => *id,
            DatabaseItemID::UserData => 1,
            DatabaseItemID::UserStats => 1,
        };
        let end_id = match end {
            DatabaseItemID::AccessMode(id) => *id,
            DatabaseItemID::Chat(id) => *id,
            DatabaseItemID::Device(id) => *id,
            DatabaseItemID::File(id) => *id,
            DatabaseItemID::Folder(id) => *id,
            DatabaseItemID::Tag(id) => *id,
            DatabaseItemID::ChatConfiguration(id) => *id,
            DatabaseItemID::Memory(id) => *id,
            DatabaseItemID::Notification(id) => *id,
            DatabaseItemID::Job(id) => *id,
            DatabaseItemID::Media(media) => 0,
            DatabaseItemID::UserData => 0,
            DatabaseItemID::UserStats => 0,
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
            DatabaseItemID::Memory(id) => Some(DatabaseItemID::Memory(id + 1)),
            DatabaseItemID::Notification(id) => Some(DatabaseItemID::Notification(id + 1)),
            DatabaseItemID::Job(id) => Some(DatabaseItemID::Job(id + 1)),
            DatabaseItemID::UserData => None,
            DatabaseItemID::UserStats => None,
            DatabaseItemID::Media(med) => None
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
            DatabaseItemID::Memory(id) => Some(DatabaseItemID::Memory(id - 1)),
            DatabaseItemID::Notification(id) => Some(DatabaseItemID::Notification(id - 1)),
            DatabaseItemID::Job(id) => Some(DatabaseItemID::Job(id - 1)),
            DatabaseItemID::UserData => None,
            DatabaseItemID::UserStats => None,
            DatabaseItemID::Media(med) => None
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
    Remove(DatabaseItemID),
    ToolRequest(ToolRequest),
    NewAuthKey,
    VerifyAuthKey(String),
    Save
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ToolRequest {
    MemoryRequest(MemoryRequest),
    UpdateExistingChatContext(ChatID, WholeContext),
    UpdateChatTitle(ChatID, Option<String>),
    UpdateChatTags(ChatID, HashSet<TagID>),
    SearchTagsByAccessModes(HashSet<AccessModeID>),
    AddTagToAccessMode(AccessModeID, TagID),
    GetLastXJobs(usize, HashSet<AccessModeID>),
    UpdatePersistentMemoryFor(AccessModeID, String),
    GetPersistentMemoryFor(AccessModeID),
    GetAutoMemoryFor(AccessModeID, usize),
    GetMediaWithoutData(MediaHash),
    UpdateAccessModeSettings(AccessModeID, HashMap<String, AMSetting>)
}

pub enum InternalDBReq {
    Database(DatabaseRequest),
    Tunnel(TunnelRequest)
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

pub struct TunnelRequest {
    response_sender:Sender<Receiver<ClientUpdate>>,
    auth_key:String,
}

impl TunnelRequest {
    pub fn new(auth_key:String) -> (Self, Receiver<Receiver<ClientUpdate>>) {
        let (response_sender, response_receiver) = channel();
        (
            Self {
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
    UnknownUpdates {updates:Vec<ClientUpdate>},
}

#[derive(Clone, Serialize, Deserialize)]
pub enum DatabaseReplyVariant {
    RequestExecuted,
    AddedItem(DatabaseItemID),
    ReturnedItem(DatabaseItem),
    ReturnedManyItems(Vec<DatabaseItem>),
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
    SavingError,
    ItemNotFound(DatabaseItemID),
    NoPersistentMemory,
    ItemNotDeletable(DatabaseItemID)
}

pub struct DatabaseReply {
    pub variant:DatabaseReplyVariant
}

pub struct ClientSessionData {
    pending_updates_send:Sender<ClientUpdate>,
    pending_updates_recv:Receiver<ClientUpdate>,
    last_decrease:DateTime<Utc>,
    last_len:usize,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ClientUpdate {
    ItemUpdate(DatabaseItemID, DatabaseItem),
    ItemRemoval(DatabaseItemID)
}

pub struct DatabaseHandler {
    priority_request_rcv:Receiver<InternalDBReq>,
    request_rcv:Receiver<InternalDBReq>,
    database:ProxDatabase,
    auth_sessions:HashMap<String, ClientSessionData>,
    auth_sessions_rng:StdRng,
    changed_since_last_save:bool,
    jobs_sender:std::sync::mpsc::Sender<Job>
}

static LOCAL_AUTHKEY:LazyLock<String> = LazyLock::new(|| {
    let mut rng = StdRng::from_os_rng();
    format!("{}", rng.next_u64())
});


impl DatabaseHandler {
    pub fn new(priority_request_rcv:Receiver<InternalDBReq>, request_rcv:Receiver<InternalDBReq>, database:ProxDatabase, jobs_sender:std::sync::mpsc::Sender<Job>) -> Self {
        Self { priority_request_rcv, request_rcv, database, auth_sessions:HashMap::with_capacity(32), auth_sessions_rng:StdRng::from_os_rng(), changed_since_last_save:true, jobs_sender }
    }
    pub fn handling_loop(&mut self) {
        for (_, job) in &self.database.jobs.jobs {
            self.jobs_sender.send(job.clone()).unwrap();
        }
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
        response_sender.send(self.database.get_request(id))
    }
    fn handle_update_request(&mut self, item:DatabaseItem, response_sender:Sender<DatabaseReply>, auth_key:Option<String>) -> Result<(), SendError<DatabaseReply>> {
        
        let mut remove_clients = Vec::with_capacity(self.auth_sessions.len());
        match auth_key {
            Some(key) if !item.get_id().is_media() => for (user, data) in self.auth_sessions.iter_mut() {
                if user != &key {
                    if data.last_len == data.pending_updates_send.len() && Utc::now().signed_duration_since(data.last_decrease) > TimeDelta::days(3) {
                        remove_clients.push(user.clone());
                    }
                    else {
                        data.last_decrease = Utc::now();
                    }
                    data.pending_updates_send.send(ClientUpdate::ItemUpdate(item.get_id(), item.clone())).unwrap();
                    data.last_len = data.pending_updates_send.len();
                }
            },
            _ => {
                let mut item = item.clone();
                if let DatabaseItem::Media(_, data) = &mut item {
                    *data = Base64EncodedString::new(vec![]);
                }
                for (user, data) in self.auth_sessions.iter_mut() {
                    if data.last_len == data.pending_updates_send.len() && Utc::now().signed_duration_since(data.last_decrease) > TimeDelta::days(3) {
                        remove_clients.push(user.clone());
                    }
                    else {
                        data.last_decrease = Utc::now();
                    }
                    data.pending_updates_send.send(ClientUpdate::ItemUpdate(item.get_id(), item.clone())).unwrap();
                    data.last_len = data.pending_updates_send.len();
                }
            }
        }
        for client in remove_clients {
            self.auth_sessions.remove(&client);
        }
        self.changed_since_last_save = true;
        response_sender.send(self.database.update_request(item))
    }
    fn handle_add_request(&mut self, item:DatabaseItem, response_sender:Sender<DatabaseReply>, auth_key:Option<String>) -> Result<(), SendError<DatabaseReply>> {
        self.changed_since_last_save = true;
        let mut s_item = item.clone();
        let (res, id) = self.database.add_request(item);
        s_item.set_id(id.clone());
        let mut remove_clients = Vec::with_capacity(self.auth_sessions.len());
        match auth_key {
            Some(key) if !id.is_media() => for (user, data) in self.auth_sessions.iter_mut() {
                if user != &key {
                    if data.last_len == data.pending_updates_send.len() && Utc::now().signed_duration_since(data.last_decrease) > TimeDelta::days(3) {
                        remove_clients.push(user.clone());
                    }
                    else {
                        data.last_decrease = Utc::now();
                    }
                    data.pending_updates_send.send(ClientUpdate::ItemUpdate(id.clone(), s_item.clone())).unwrap();
                    data.last_len = data.pending_updates_send.len();
                }
            },
            _ => {
                if let DatabaseItem::Media(_, data) = &mut s_item {
                    *data = Base64EncodedString::new(vec![]);
                }
                for (user, data) in self.auth_sessions.iter_mut() {
                    if data.last_len == data.pending_updates_send.len() && Utc::now().signed_duration_since(data.last_decrease) > TimeDelta::days(3) {
                        remove_clients.push(user.clone());
                    }
                    else {
                        data.last_decrease = Utc::now();
                    }
                    data.pending_updates_send.send(ClientUpdate::ItemUpdate(id.clone(), s_item.clone())).unwrap();
                    data.last_len = data.pending_updates_send.len();
                }
            }
        }
        for client in remove_clients {
            self.auth_sessions.remove(&client);
        }
        match s_item.clone() {
            DatabaseItem::Job(mut job) => if let DatabaseItemID::Job(job_id) = id {job.id = job_id; println!("[database] Sending job to the job thread"); self.jobs_sender.send(job).unwrap();}
            _ => ()
        }
        response_sender.send(res)
    }
    fn handle_new_auth_key(&mut self, response_sender:Sender<DatabaseReply>) -> Result<(), SendError<DatabaseReply>> {
        let new_auth = self.auth_sessions_rng.next_u64().to_string();
        let (send, recv) = channel();
        self.auth_sessions.insert(new_auth.clone(), ClientSessionData { pending_updates_send: send, pending_updates_recv:recv, last_decrease:Utc::now(), last_len:0 });
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
                    self.database.devices.get_devices().get(&(self.database.devices.latest_id - 1)).map(|item| {DatabaseItem::Device(item.clone())}),
                    self.database.access_modes.get_modes().get(&(self.database.access_modes.latest_id - 1)).map(|item| {DatabaseItem::AccessMode(item.clone())}),
                    self.database.chats.get_last_chat().map(|item| {DatabaseItem::Chat(item.clone())}),
                    self.database.folders.get_last_folder().map(|item| {DatabaseItem::Folder(item.clone())}),
                    self.database.files.get_last_file().map(|item| {DatabaseItem::File(item.clone())}),
                    self.database.tags.get_last_tag().map(|item| {DatabaseItem::Tag(item.clone())}),
                    Some(DatabaseItem::UserData(self.database.personal_info.user_data.clone())),

                ] }
                ) })
            },
            DatabaseInfoRequest::UnknownUpdates { access_key } => {
                panic!("UnknownUpdates can't reach here")
                /*response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::Info(
                DatabaseInfoReply::UnknownUpdates { updates: self.auth_sessions.get_mut(&access_key).unwrap().pending_updates.drain(..).collect() }
                ) })*/
            }
        }
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

    fn handle_tool_request(&mut self, request:ToolRequest, response_sender:Sender<DatabaseReply>) -> Result<(), SendError<DatabaseReply>> {
        match request {
            ToolRequest::MemoryRequest(memory_request) => {
                let memories = self.database.memories.retrieve_data_from_ids(self.database.memories.retrieve_ids(memory_request), self.database.database_folder.clone());
                response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::ReturnedManyItems(memories.into_iter().map(|(memory, data)| {DatabaseItem::Memory(memory, data)}).collect()) })
            },
            ToolRequest::UpdateExistingChatContext(chat_id, new_context) => {
                self.database.chats.get_chats_mut().get_mut(&chat_id).map(|chat| {
                    chat.context = new_context;
                    chat.latest_message = Utc::now();
                    for (user, data) in self.auth_sessions.iter_mut() {
                        data.pending_updates_send.send(ClientUpdate::ItemUpdate(DatabaseItemID::Chat(chat_id), DatabaseItem::Chat(chat.clone())));
                    }
                });

                response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted})
            },
            ToolRequest::UpdateChatTitle(chat_id, new_title) => {
                self.database.chats.get_chats_mut().get_mut(&chat_id).map(|chat| {
                    chat.chat_title = new_title;
                    chat.latest_message = Utc::now();
                    for (user, data) in self.auth_sessions.iter_mut() {
                        data.pending_updates_send.send(ClientUpdate::ItemUpdate(DatabaseItemID::Chat(chat_id), DatabaseItem::Chat(chat.clone())));
                    }
                });

                response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted})
            },
            ToolRequest::SearchTagsByAccessModes(access_modes) => {
                let mut tags = Vec::with_capacity(128); 
                for mode in access_modes {
                    self.database.access_modes.get_modes().get(&mode).map(|am| {
                        am.tags.iter().for_each(|tag_id| {
                            self.database.tags.get_tag_from_tagid(*tag_id).map(|tag| {
                                tags.push(DatabaseItem::Tag(tag.clone()));
                            });
                        })
                    });
                }
                
                response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::ReturnedManyItems(tags)})
            } ,
            ToolRequest::AddTagToAccessMode(access_mode_id, tag_id) => {
                self.database.access_modes.get_modes_mut().get_mut(&access_mode_id).map(|access_mode| {
                    access_mode.tags.insert(tag_id);
                    for (user, data) in self.auth_sessions.iter_mut() {
                        data.pending_updates_send.send(ClientUpdate::ItemUpdate(DatabaseItemID::AccessMode(access_mode_id), DatabaseItem::AccessMode(access_mode.clone())));
                    }
                });
                
                
                response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted})
            },
            ToolRequest::UpdateChatTags(chat_id, tags) => {
                self.database.chats.get_chats_mut().get_mut(&chat_id).map(|chat| {
                    chat.tags = tags;
                    for (user, data) in self.auth_sessions.iter_mut() {
                        data.pending_updates_send.send(ClientUpdate::ItemUpdate(DatabaseItemID::Chat(chat_id), DatabaseItem::Chat(chat.clone())));
                    }
                });

                response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted})
            },
            ToolRequest::GetLastXJobs(number, access_modes) => {
                let mut jobs = Vec::with_capacity(number);
                for job_id in (0..self.database.jobs.latest_job_id).rev() {
                    if let Some(job) = self.database.jobs.get_job(job_id) && job.access_modes.intersection(&access_modes).count() > 0 {
                        if jobs.len() < number {
                            jobs.push(DatabaseItem::Job(job.clone()));
                        }
                        else {
                            break;
                        }
                    }
                }
                response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::ReturnedManyItems(jobs)})
            },
            ToolRequest::UpdatePersistentMemoryFor(access_mode_id, new_data) => {
                if let Some(access_mode) = self.database.access_modes.get_modes_mut().get_mut(&access_mode_id) {
                    if let Some(memory_id) = access_mode.persistent_memory {
                        self.database.memories.update_memory(memory_id, new_data, self.database.database_folder.clone());
                        let new_mem = self.database.memories.get_memory_with_data(memory_id, self.database.database_folder.clone()).unwrap();
                        for (user, data) in self.auth_sessions.iter_mut() {
                            data.pending_updates_send.send(ClientUpdate::ItemUpdate(DatabaseItemID::Memory(memory_id), DatabaseItem::Memory(new_mem.0.clone(), new_mem.1.clone())));
                        }

                        response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted})

                    }
                    else {
                        let memory_id = self.database.memories.add_memory(new_data, HashSet::from([0, access_mode_id]), HashSet::new(), self.database.database_folder.clone(), memories::MemoryKind::Persistent);
                        let new_mem = self.database.memories.get_memory_with_data(memory_id, self.database.database_folder.clone()).unwrap();
                        access_mode.persistent_memory = Some(memory_id);
                        for (user, data) in self.auth_sessions.iter_mut() {
                            data.pending_updates_send.send(ClientUpdate::ItemUpdate(DatabaseItemID::AccessMode(access_mode_id), DatabaseItem::AccessMode(access_mode.clone())));
                            data.pending_updates_send.send(ClientUpdate::ItemUpdate(DatabaseItemID::Memory(memory_id), DatabaseItem::Memory(new_mem.0.clone(), new_mem.1.clone())));
                        }

                        response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted})

                    }
                }
                else {
                    response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotFound(DatabaseItemID::AccessMode(access_mode_id)))})
                }
            },
            ToolRequest::GetPersistentMemoryFor(access_mode_id) => {
                if let Some(access_mode) = self.database.access_modes.get_modes_mut().get_mut(&access_mode_id) {
                    if let Some(memory_id) = access_mode.persistent_memory {
                        let new_mem = self.database.memories.get_memory_with_data(memory_id, self.database.database_folder.clone()).unwrap();
                        response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::Memory(new_mem.0.clone(), new_mem.1.clone()))})
                    }   
                    else {
                        response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::NoPersistentMemory)})  
                    }
                }
                else {
                    response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotFound(DatabaseItemID::AccessMode(access_mode_id)))})
                }

            },
            ToolRequest::GetAutoMemoryFor(access_mode_id, max_last_memories) => {
                if let Some(access_mode) = self.database.access_modes.get_modes_mut().get_mut(&access_mode_id) {
                    let persistent = if let Some(memory_id) = access_mode.persistent_memory {
                        let new_mem = self.database.memories.get_memory_with_data(memory_id, self.database.database_folder.clone()).unwrap();
                        DatabaseItem::Memory(new_mem.0.clone(), new_mem.1)
                    }   
                    else {
                        DatabaseItem::AccessMode(access_mode.clone())
                    };
                    let fleeting = self.database.memories.retrieve_ids(MemoryRequest::new(Utc::now() - TimeDelta::weeks(12000), Utc::now(), HashSet::from([access_mode_id]), None, MemReqMax::MaxRecentFirst(10)));
                    let fleeting = self.database.memories.retrieve_data_from_ids(fleeting, self.database.database_folder.clone());
                    let mut fleeting = fleeting.iter().map(|(mem, txt)| {DatabaseItem::Memory(mem.clone(), txt.clone())}).collect::<Vec<DatabaseItem>>();
                    fleeting.insert(0, persistent);
                    response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::ReturnedManyItems(fleeting)})
                }
                else {
                    response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotFound(DatabaseItemID::AccessMode(access_mode_id)))})
                }
            },
            ToolRequest::GetMediaWithoutData(media_hash) => {
                if let Some(media) = self.database.media.get_media(&media_hash) {
                    response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::ReturnedItem(DatabaseItem::Media(media.clone(), Base64EncodedString::new(vec![])))})
                }
                else {
                    response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotFound(DatabaseItemID::Media(media_hash)))})
                }
            },
            ToolRequest::UpdateAccessModeSettings(access_mode_id, new_settings) => {
                if let Some(access_mode) = self.database.access_modes.get_modes_mut().get_mut(&access_mode_id) {
                    access_mode.am_settings = new_settings;
                    response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::RequestExecuted})
                }
                else {
                    response_sender.send(DatabaseReply { variant: DatabaseReplyVariant::Error(DatabaseError::ItemNotFound(DatabaseItemID::AccessMode(access_mode_id)))})
                }
            }
        }
    }

    fn handle_remove_request(&mut self, id:DatabaseItemID, response_sender:Sender<DatabaseReply>, auth_key:Option<String>) -> Result<(), SendError<DatabaseReply>> {
        
        let mut remove_clients = Vec::with_capacity(self.auth_sessions.len());
        match auth_key {
            Some(key) => for (user, data) in self.auth_sessions.iter_mut() {
                if user != &key {
                    if data.last_len == data.pending_updates_send.len() && Utc::now().signed_duration_since(data.last_decrease) > TimeDelta::days(3) {
                        remove_clients.push(user.clone());
                    }
                    else {
                        data.last_decrease = Utc::now();
                    }
                    data.pending_updates_send.send(ClientUpdate::ItemRemoval(id.clone())).unwrap();
                    data.last_len = data.pending_updates_send.len();
                }
            },
            None => for (user, data) in self.auth_sessions.iter_mut() {
                if data.last_len == data.pending_updates_send.len() && Utc::now().signed_duration_since(data.last_decrease) > TimeDelta::days(3) {
                    remove_clients.push(user.clone());
                }
                else {
                    data.last_decrease = Utc::now();
                }
                data.pending_updates_send.send(ClientUpdate::ItemRemoval(id.clone())).unwrap();
                data.last_len = data.pending_updates_send.len();
            }
        }
        for client in remove_clients {
            self.auth_sessions.remove(&client);
        }
        response_sender.send(self.database.remove_request(id.clone()))
    }

    fn handle_request(&mut self, request:InternalDBReq) -> Result<(), SendError<DatabaseReply>> {
        match request {
            InternalDBReq::Database(db_request) => 
                match db_request.variant {
                    DatabaseRequestVariant::Get(id) => self.handle_get_request(id, db_request.response_sender),
                    DatabaseRequestVariant::Add(item) => self.handle_add_request(item, db_request.response_sender, db_request.auth_key),
                    DatabaseRequestVariant::Update(item) => self.handle_update_request(item, db_request.response_sender, db_request.auth_key),
                    DatabaseRequestVariant::Remove(id) => self.handle_remove_request(id, db_request.response_sender, db_request.auth_key),
                    DatabaseRequestVariant::NewAuthKey => self.handle_new_auth_key(db_request.response_sender),
                    DatabaseRequestVariant::VerifyAuthKey(auth) => self.handle_auth_verification(auth, db_request.response_sender),
                    DatabaseRequestVariant::Info(info_request) => self.handle_info_request(info_request, db_request.response_sender),
                    DatabaseRequestVariant::GetAll => self.handle_getall(db_request.response_sender),
                    DatabaseRequestVariant::Save => self.handle_save(db_request.response_sender),
                    DatabaseRequestVariant::ToolRequest(tool_request) => self.handle_tool_request(tool_request, db_request.response_sender)

                },
            InternalDBReq::Tunnel(tunnel_req) => {
                self.auth_sessions.get(&tunnel_req.auth_key).map(|auth_session| {
                    tunnel_req.response_sender.send(auth_session.pending_updates_recv.clone()).unwrap();
                });
                Ok(())
            }
        }
        
    }
}
#[derive(Clone)]
pub struct DatabaseSender {
    prio_queue:Sender<InternalDBReq>,
    normal_queue:Sender<InternalDBReq>,
}

impl DatabaseSender {
    pub fn send_normal(&self, req:DatabaseRequest) {
        self.normal_queue.send(InternalDBReq::Database(req));
    }
    pub fn send_prio(&self, req:DatabaseRequest) {
        self.prio_queue.send(InternalDBReq::Database(req));
    }
    pub fn send_prio_tunnel(&self, req:TunnelRequest) {
        self.prio_queue.send(InternalDBReq::Tunnel(req));
    }
}

pub fn launch_database_thread(database:ProxDatabase) -> (DatabaseSender, std::sync::mpsc::Receiver<Job>) {
    let (prio_send, prio_rcv) = channel();
    let (normal_send, normal_rcv) = channel();
    let (job_send, job_recv) = std::sync::mpsc::channel();
    thread::spawn(move || {
        DatabaseHandler::new(prio_rcv, normal_rcv, database, job_send).handling_loop();
    });
    (DatabaseSender { prio_queue:prio_send, normal_queue:normal_send }, job_recv)
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