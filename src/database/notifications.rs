use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::database::{DatabaseItemID, access_modes::AccessModeID};

pub type NotificationID = usize;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NotificationReason {
    ChatRoundFinished,

}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Notification {
    pub timestamp:DateTime<Utc>,
    pub related_item:Option<DatabaseItemID>,
    pub access_modes:HashSet<AccessModeID>,
    pub id:NotificationID,
    pub reason:NotificationReason,
    pub text:Option<String>,
}

impl Notification {
    pub fn new(related_item:Option<DatabaseItemID>, access_modes:HashSet<AccessModeID>, reason:NotificationReason, text:Option<String>) -> Self {
        Self { timestamp: Utc::now(), related_item, access_modes, id: 0, reason, text }
    }
    
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Notifications {
    pub notifs:HashMap<NotificationID, Notification>,
    pub latest_id:NotificationID,
}

impl Notifications {
    pub fn new() -> Self {
        Self { notifs: HashMap::with_capacity(32), latest_id: 0 }
    }
    pub fn add_notification(&mut self, mut notification:Notification) -> NotificationID {
        notification.id = self.latest_id;
        let id = notification.id;
        self.latest_id += 1;
        self.notifs.insert(id, notification);
        id
    }
    pub fn remove_notification(&mut self, id:NotificationID) -> bool { // was there
        self.notifs.remove(&id).is_some()
    }
    pub fn insert_notification_raw(&mut self, notification:Notification) {
        self.notifs.insert(notification.id, notification);
    }
    pub fn get_notifications(&self) -> &HashMap<NotificationID, Notification> {
        &self.notifs
    }
}
