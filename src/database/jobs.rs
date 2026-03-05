use std::collections::HashSet;

use chrono::{DateTime, TimeDelta, Utc};

use crate::{ai_interaction::AiEndpointSender, database::{DatabaseItemID, DatabaseRequest, DatabaseSender, access_modes::AccessModeID, chats::ChatID, configuration::ChatConfigID, context::WholeContext, notifications::{Notification, NotificationReason}}};

pub type JobID = usize;

pub struct Job {
    added_at:DateTime<Utc>,
    timing:JobTiming,
    repeat:JobRepeat,
    job_type:JobType,
    description:Option<String>,
    access_modes:HashSet<AccessModeID>,
    id:JobID
}

pub enum JobTiming {
    OnTime{time:DateTime<Utc>},
    ASAP,
    InDrought{max_timeout:TimeDelta}
}

pub enum JobRepeat {
    No,
    RegularInterval(TimeDelta),
    RegularTimeOfDay(TimeDelta),

}

pub enum JobType {
    Reminder, // Will send a notification
    Check(Vec<String>), // Will send a notification with a checklist
    Title(ChatID),
    Tag(DatabaseItemID),
    Callback(ChatConfigID),
    EvolvingCallback {
        config:ChatConfigID,
        initial_prompt:WholeContext,
        scratchpad:WholeContext,
    }
}

impl Job {
    pub fn execute(&mut self, database_sender:DatabaseSender, ai_endpoint:AiEndpointSender) {
        match self.job_type {
            JobType::Reminder => {
                let notif = Notification::new(None, self.access_modes.clone(), NotificationReason::Reminder, self.description.clone());
                let (db_req, db_recv) = DatabaseRequest::new(super::DatabaseRequestVariant::Add(super::DatabaseItem::Notification(notif)), None);
                database_sender.send_prio(db_req);
            },
            _ => ()
        }
    }
}