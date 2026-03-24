use std::{collections::{HashMap, HashSet}, sync::mpsc::{Receiver, RecvTimeoutError}, thread, time::Duration};

use chrono::{Date, DateTime, Days, NaiveTime, TimeDelta, Utc};
use html_parser::{Dom, Node};
use serde::{Deserialize, Serialize};

use crate::{ai_interaction::{AiEndpointSender, endpoint_api::{EndpointRequest, EndpointRequestVariant, EndpointResponse, EndpointResponseVariant}, tools::ProximaTool}, database::{DatabaseItem, DatabaseItemID, DatabaseReply, DatabaseReplyVariant, DatabaseRequest, DatabaseSender, ToolRequest, access_modes::AccessModeID, chats::{Chat, ChatID, SessionType}, configuration::ChatConfigID, context::{ContextData, ContextPart, ContextPosition, WholeContext}, description::Description, notifications::{Notification, NotificationReason}, tags::{NewTag, Tag}, user::UserStats}};

pub type JobID = usize;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Job {
    pub added_at:DateTime<Utc>,
    last_executed:Option<DateTime<Utc>>,
    pub timing:JobTiming,
    pub repeat:JobRepeat,
    pub job_type:JobType,
    pub description:Option<String>,
    pub access_modes:HashSet<AccessModeID>,
    pub id:JobID
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobTiming {
    OnTime{time:DateTime<Utc>},
    ASAP,
    InDrought{max_timeout:TimeDelta}
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobRepeat {
    No,
    RegularInterval(TimeDelta),
    RegularTimeOfDay(TimeDelta),

}

impl JobRepeat {
    pub fn must_repeat(&self) -> bool {
        match self {
            JobRepeat::No => false,
            _ => true,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    pub fn new(timing:JobTiming, repeat:JobRepeat, job_type:JobType, description:Option<String>, access_modes:HashSet<AccessModeID>) -> Self {
        Self { added_at: Utc::now(), last_executed: None, timing, repeat, job_type, description, access_modes, id: 0 }
    }
    pub fn execute(&mut self, database_sender:DatabaseSender, ai_endpoint:AiEndpointSender) -> JobExecution {
        match &self.job_type {
            JobType::Reminder => {
                let notif = Notification::new(None, self.access_modes.clone(), NotificationReason::Reminder, self.description.clone());
                let (db_req, db_recv) = DatabaseRequest::new(super::DatabaseRequestVariant::Add(super::DatabaseItem::Notification(notif)), None);
                database_sender.send_prio(db_req);
                if let Ok(DatabaseReply { variant:DatabaseReplyVariant::AddedItem(_) }) = db_recv.recv() {
                    self.last_executed = Some(Utc::now());
                    JobExecution::Success { must_reschedule: self.repeat.must_repeat() }
                }
                else {
                    JobExecution::Failure { must_reschedule: true }
                }
            },
            JobType::Check(checklist) => {
                let notif = Notification::new(None, self.access_modes.clone(), NotificationReason::Checklist(checklist.clone()), self.description.clone());
                let (db_req, db_recv) = DatabaseRequest::new(super::DatabaseRequestVariant::Add(super::DatabaseItem::Notification(notif)), None);
                database_sender.send_prio(db_req);
                if let Ok(DatabaseReply { variant:DatabaseReplyVariant::AddedItem(_) }) = db_recv.recv() {
                    self.last_executed = Some(Utc::now());
                    JobExecution::Success { must_reschedule: self.repeat.must_repeat() }
                }
                else {
                    JobExecution::Failure { must_reschedule: true }
                }
            },
            JobType::Title(chat_id) => {
                let (db_req, db_recv) = DatabaseRequest::new(super::DatabaseRequestVariant::Get(DatabaseItemID::Chat(*chat_id)), None);
                database_sender.send_prio(db_req);
                if let Ok(DatabaseReply {variant:DatabaseReplyVariant::ReturnedItem(DatabaseItem::Chat(chat))}) = db_recv.recv() {
                    let mut total_text = String::new();
                    let mut total_char_len = 0;
                    for part in chat.context.get_parts() {
                        match part.get_position() {
                            ContextPosition::System => (),
                            _ => {
                                let text = part.data_to_single_text();
                                let text_len = text.chars().collect::<Vec<char>>().len();
                                if total_char_len + text_len < 5000 {
                                    total_text += &text;
                                    total_char_len += text_len;
                                }
                                else {
                                    break;
                                }
                            }
                        }
                    }
                    let context = WholeContext::new(vec![
                        ContextPart::new(vec![
                            ContextData::Text(String::from(include_str!("../../configuration/prompts/title.txt")))
                        ], ContextPosition::System),
                        ContextPart::new(vec![
                            ContextData::Text(format!("<user_conversation>\n{total_text}\n</user_conversation>"))
                        ], ContextPosition::User)
                    ]);
                    let mut final_title = None;
                    'title_tries:for i in 0..5 {
                        let (ai_request, ai_recv) = EndpointRequest::new(
                        EndpointRequestVariant::RespondToFullPrompt { whole_context: context.clone(), streaming: false, session_type: SessionType::Function, chat_settings: None, chat_id: None, access_mode: 0 }
                        );
                        ai_endpoint.send_prio(ai_request);
                        if let Ok(EndpointResponse { variant:EndpointResponseVariant::Block(response) }) = ai_recv.recv() {
                            let mut str = response.data_to_single_text();
                            if !str.contains("</conversation_title>") {
                                str += "\n</conversation_title>"
                            }
                            if let Ok(dom) = Dom::parse(&str) && dom.children.len() > 0 {
                                for child in dom.children {
                                    if let Some(element) = child.element() && element.name == "conversation_title" && let Some(Node::Text(title)) = element.children.get(0) && title.len() > 3 {
                                        final_title = Some(title.clone());
                                        break 'title_tries;
                                    }
                                }
                            }
                        }
                    }
                    if let Some(title) = final_title {
                        let (db_req, db_recv) = DatabaseRequest::new(super::DatabaseRequestVariant::ToolRequest(ToolRequest::UpdateChatTitle(*chat_id, Some(title))), None);
                        database_sender.send_prio(db_req);
                        if let Ok(DatabaseReply {variant:DatabaseReplyVariant::RequestExecuted}) = db_recv.recv() {
                            self.last_executed = Some(Utc::now());
                            JobExecution::Success { must_reschedule: false }        
                        }
                        else {
                            JobExecution::Failure { must_reschedule: true }
                        }
                    }
                    else {
                        JobExecution::Failure { must_reschedule: true }
                    }
                    // send ai endpoint request to get it titled and update the DB
                }
                else {
                    JobExecution::Failure { must_reschedule: true }
                }
            },
            JobType::Tag(item_id) => {
                let (db_req, db_recv) = DatabaseRequest::new(super::DatabaseRequestVariant::Get(item_id.clone()), None);
                database_sender.send_prio(db_req);
                if let Ok(DatabaseReply {variant:DatabaseReplyVariant::ReturnedItem(item)}) = db_recv.recv() {
                    match item {
                        DatabaseItem::Chat(chat) => {
                            let mut total_text = String::new();
                            let mut total_char_len = 0;
                            for part in chat.context.get_parts() {
                                match part.get_position() {
                                    ContextPosition::System => (),
                                    _ => {
                                        let text = part.data_to_single_text();
                                        let text_len = text.chars().collect::<Vec<char>>().len();
                                        if total_char_len + text_len < 5000 {
                                            total_text += &text;
                                            total_char_len += text_len;
                                        }
                                        else {
                                            break;
                                        }
                                    }
                                }
                            }
                            
                            let (db_req, db_recv) = DatabaseRequest::new(super::DatabaseRequestVariant::ToolRequest(ToolRequest::SearchTagsByAccessModes(chat.access_modes.clone())), None);
                            database_sender.send_prio(db_req);
                            if let Ok(DatabaseReply { variant:DatabaseReplyVariant::ReturnedManyItems(items) }) = db_recv.recv() {
                                let mut existing_tag_names = HashMap::with_capacity(16);
                                let existing_tags = items.iter().map(|tag| {
                                    match tag {
                                        DatabaseItem::Tag(tag) => {
                                            existing_tag_names.insert(tag.get_name().clone(), tag.get_id());
                                            format!("{}\n", tag.get_name())
                                        },
                                        _ => panic!("Should only be tags")
                                    }
                                }).collect::<Vec<String>>().concat();
                                let context = WholeContext::new(vec![
                                    ContextPart::new(vec![
                                        ContextData::Text(String::from(include_str!("../../configuration/prompts/tag.txt")))
                                    ], ContextPosition::System),
                                    ContextPart::new(vec![
                                        ContextData::Text(format!("<existing_tags>\n{existing_tags}\n</existing_tags>")),
                                        ContextData::Text(format!("<user_conversation>\n{total_text}\n</user_conversation>"))
                                    ], ContextPosition::User)
                                ]);
                                
                                let mut tag_names = Vec::with_capacity(16);
                                'title_tries:for i in 0..5 {
                                    let (ai_request, ai_recv) = EndpointRequest::new(
                                    EndpointRequestVariant::RespondToFullPrompt { whole_context: context.clone(), streaming: false, session_type: SessionType::Function, chat_settings: None, chat_id: None, access_mode: 0 }
                                    );
                                    ai_endpoint.send_prio(ai_request);
                                    if let Ok(EndpointResponse { variant:EndpointResponseVariant::Block(response) }) = ai_recv.recv() {
                                        let mut str = response.data_to_single_text();
                                        if !str.contains("</conversation_tags>") {
                                            str += "\n</conversation_tags>"
                                        }
                                        if let Ok(dom) = Dom::parse(&str) && dom.children.len() > 0 {
                                            for child in dom.children {
                                                if let Some(element) = child.element() && element.name == "conversation_tags" && let Some(Node::Text(tags_str)) = element.children.get(0) && tags_str.len() > 3 {
                                                    let mut added_tags = 0;
                                                    for tag_line in tags_str.lines() {
                                                        if tag_line.len() > 1 {
                                                            added_tags += 1;
                                                            tag_names.push(tag_line.trim().to_string());
                                                        }
                                                    }
                                                    if added_tags > 0 {
                                                        break 'title_tries;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                let mut chat_tags = HashSet::with_capacity(16);
                                for name in tag_names {
                                    if let Some(tag_id) = existing_tag_names.get(&name) {
                                        if chat_tags.len() < 10 {   
                                            chat_tags.insert(*tag_id);
                                        }
                                        else {
                                            break;
                                        }
                                    }
                                    else if chat_tags.len() < 10 {
                                        let new_tag = Tag::new(0, name.clone(), Description::new(String::new()), None);
                                        let (db_req, db_recv) = DatabaseRequest::new(super::DatabaseRequestVariant::Add(DatabaseItem::Tag(new_tag)), None);
                                        database_sender.send_prio(db_req);
                                        if let Ok(DatabaseReply { variant:DatabaseReplyVariant::AddedItem(DatabaseItemID::Tag(tag_id)) }) = db_recv.recv() {
                                            chat_tags.insert(tag_id);
                                            for access_mode in &chat.access_modes {
                                                let (db_req, db_recv) = DatabaseRequest::new(super::DatabaseRequestVariant::ToolRequest(ToolRequest::AddTagToAccessMode(*access_mode, tag_id)), None);
                                                database_sender.send_prio(db_req);
                                            }
                                            existing_tag_names.insert(name, tag_id);
                                        }
                                    }
                                    else {
                                        break;
                                    }
                                }
                                let (db_req, db_recv) = DatabaseRequest::new(super::DatabaseRequestVariant::ToolRequest(ToolRequest::UpdateChatTags(chat.id, chat_tags)), None);
                                database_sender.send_prio(db_req);
                                if let Ok(DatabaseReply { variant:DatabaseReplyVariant::RequestExecuted }) = db_recv.recv() {
                                    self.last_executed = Some(Utc::now());
                                    JobExecution::Success { must_reschedule: false }
                                }
                                else {
                                    JobExecution::Failure { must_reschedule: true }
                                }
                                
                            }
                            else {
                                JobExecution::Failure { must_reschedule: true }
                            }
                            
                            
                        },
                        _ => JobExecution::Failure { must_reschedule: false }
                    }   
                }
                else {
                    JobExecution::Failure { must_reschedule: true }
                }
            }
            JobType::Callback(config) => {
                let (db_req, db_recv) = DatabaseRequest::new(super::DatabaseRequestVariant::Get(DatabaseItemID::ChatConfiguration(*config)), None);
                database_sender.send_prio(db_req);
                if let Ok(DatabaseReply { variant:DatabaseReplyVariant::ReturnedItem(DatabaseItem::ChatConfig(conf)) }) = db_recv.recv() {
                    let current_time = format!("{}", Utc::now());
                    let mut default_text = include_str!("../../configuration/prompts/callback.txt").to_string();
                    default_text = default_text.replace("CURRENT_TIME", current_time.trim());
                    default_text = match self.repeat {
                        JobRepeat::No => default_text.replace("REPEAT_STATEMENT", ""),
                        _ => match conf.get_tools() {
                            Some(tools) => {
                                let mut addition = format!("\nThis job repeats on a regular schedule");
                                if tools.get_used_tools().contains(&ProximaTool::Jobs) {
                                    addition += ", in order to stop this, you can use the Job tool";
                                }
                                if tools.get_used_tools().contains(&ProximaTool::Memory) {
                                    addition += ", in order to carry memory between each call, you can use the memory tool";
                                }
                                default_text.replace("REPEAT_STATEMENT", &addition)
                            },
                            None => default_text.replace("REPEAT_STATEMENT", "\nThis job repeats on a regular schedule")
                        }
                    };
                    let starting_data = ContextData::Text(format!("{default_text}{}", self.description.clone().unwrap()));
                    let context = match conf.tools.clone() {
                        Some(tools) => {
                            WholeContext::new_with_all_settings(vec![ContextPart::new_user_prompt_with_tools(vec![starting_data])], &conf)
                        },
                        None => WholeContext::new_with_all_settings(vec![ContextPart::new(vec![starting_data], ContextPosition::User)], &conf)
                    };
                    
                    let mut chat = Chat::new_with_id(0, context.clone(), None, 0, Some(conf.clone()));
                    chat.access_modes.insert(1);
                    let (db_req, db_recv) = DatabaseRequest::new(super::DatabaseRequestVariant::Add(DatabaseItem::Chat(chat)), None);
                    database_sender.send_prio(db_req);
                    if let Ok(DatabaseReply { variant:DatabaseReplyVariant::AddedItem(DatabaseItemID::Chat(chat_id)) }) = db_recv.recv() {
                        let (ai_request, ai_recv) = EndpointRequest::new(
                        EndpointRequestVariant::RespondToFullPrompt { whole_context: context, streaming: false, session_type: SessionType::Function, chat_settings: Some(conf.clone()), chat_id: Some(chat_id), access_mode: 1 }
                        );
                        ai_endpoint.send_prio(ai_request);
                        if let Ok(EndpointResponse { variant:EndpointResponseVariant::MultiTurnBlock(new_context) }) = ai_recv.recv() {
                            JobExecution::Success { must_reschedule: self.repeat.must_repeat() }
                        }
                        else {
                            JobExecution::Failure { must_reschedule: self.repeat.must_repeat() }
                        }

                    }
                    else {
                        JobExecution::Failure { must_reschedule: self.repeat.must_repeat() }
                    }
                }
                else {
                    JobExecution::Failure { must_reschedule: self.repeat.must_repeat() }
                }
            }
            _ => JobExecution::Failure { must_reschedule: false }
        }
    }
    pub fn schedule(&self, user_stats:&mut UserStats) -> DateTime<Utc> {
        match self.timing {
            JobTiming::ASAP => Utc::now().checked_add_signed(TimeDelta::seconds(3)).unwrap(),
            JobTiming::InDrought { max_timeout } => {
                let now = Utc::now();
                let slot = user_stats.heatmap.get_first_lowest_slot(now.time().signed_duration_since(NaiveTime::from_hms_opt(0, 0, 0).unwrap()));
                if slot > max_timeout {
                    now.date_naive().and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap() + max_timeout).and_utc()
                }
                else {
                    now.date_naive().and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap() + slot).and_utc()
                }

            },
            JobTiming::OnTime { time } => match self.last_executed {
                Some(executed) => match self.repeat {
                    JobRepeat::No => panic!("Can't reschedule non-repeating job that has already been done"),
                    JobRepeat::RegularInterval(interval) => {
                        let now = Utc::now();
                        let diff = now.signed_duration_since(time);
                        let times_intervalled = diff.checked_div(interval.num_seconds() as i32).unwrap().num_seconds();
                        time + TimeDelta::seconds(times_intervalled * interval.num_seconds())
                    },
                    JobRepeat::RegularTimeOfDay(time) => {
                        let today_time = Utc::now().date_naive().and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap() + time).and_utc();
                        if today_time < executed {
                            Utc::now().checked_add_days(Days::new(1)).unwrap().date_naive().and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap() + time).and_utc()
                        }
                        else {
                            today_time
                        }
                    }
                },
                None => time
            }
        }
    }
}

pub enum JobExecution {
    Success{must_reschedule:bool},
    Failure{must_reschedule:bool},
}

pub fn schedule_job<'a>(scheduled_job:&mut Option<usize>, database_sender:DatabaseSender, jobs:&Vec<Job>) -> DateTime<Utc> {
    let (db_req, db_recv) = DatabaseRequest::new(super::DatabaseRequestVariant::Get(DatabaseItemID::UserStats), None);
    database_sender.send_prio(db_req);
    let mut scheduled_time = Utc::now().checked_add_days(Days::new(1)).unwrap();
    if let Ok(DatabaseReply { variant:DatabaseReplyVariant::ReturnedItem(DatabaseItem::UserStats(mut user_stats)) }) = db_recv.recv() {
        if jobs.len() > 0 {
            println!("[jobs] jobs to schedule, computing which goes first");
            for (i, job) in jobs.iter().enumerate() {
                match scheduled_job {
                    Some(scheduled) => if jobs[*scheduled].schedule(&mut user_stats) >= job.schedule(&mut user_stats) {
                        *scheduled = i;
                        scheduled_time = job.schedule(&mut user_stats);
                        println!("[jobs] job {} is scheduled sooner at {}", job.id, scheduled_time);
                    },
                    None => {
                        *scheduled_job = Some(i);
                        scheduled_time = job.schedule(&mut user_stats);
                        println!("[jobs] job {} is scheduled at {}", job.id, scheduled_time);
                    }
                }
            }
            scheduled_time = jobs[scheduled_job.unwrap()].schedule(&mut user_stats);
        }
        else {
            println!("[jobs] no jobs to schedule, going back to waiting");
            *scheduled_job = None;
        }
        
    }
    scheduled_time
}

pub fn get_timeout_from_deadline(deadline:DateTime<Utc>) -> Duration {
    let now = Utc::now();
    let time_delta = deadline.signed_duration_since(now);
    println!("[jobs] time delta for timeout : {}", time_delta);
    if time_delta.num_seconds() > 0 {
        Duration::from_secs(time_delta.num_seconds() as u64)
    }
    else {
        Duration::from_millis(500)
    }
}

pub fn job_thread(job_receiver:Receiver<Job>, database_sender:DatabaseSender, ai_sender:AiEndpointSender) {
    thread::spawn(move || {
        let mut jobs = Vec::with_capacity(16);
        let mut scheduled_job: Option<usize> = None;
        let mut current_deadline = Utc::now().checked_add_days(Days::new(1)).unwrap();
        loop {
            match job_receiver.recv_timeout(get_timeout_from_deadline(current_deadline)) {
                Ok(job) => {
                    println!("[jobs] Received job from database");
                    jobs.push(job);
                    current_deadline = schedule_job(&mut scheduled_job, database_sender.clone(), &mut jobs);
                    println!("[jobs] job scheduled for {}", current_deadline);
                },
                Err(error) => match error {
                    RecvTimeoutError::Disconnected => break,
                    RecvTimeoutError::Timeout => match scheduled_job {
                        Some(job) => {
                            println!("[jobs] job getting executed");
                            match jobs[job].execute(database_sender.clone(), ai_sender.clone()) {
                                JobExecution::Success { must_reschedule } => if !must_reschedule {
                                    let (db_req, db_recv) = DatabaseRequest::new(super::DatabaseRequestVariant::Remove(DatabaseItemID::Job(jobs[job].id)), None);
                                    database_sender.send_prio(db_req);
                                    jobs.remove(job);
                                    scheduled_job = None;
                                },
                                JobExecution::Failure { must_reschedule  } => if !must_reschedule {
                                    let (db_req, db_recv) = DatabaseRequest::new(super::DatabaseRequestVariant::Remove(DatabaseItemID::Job(jobs[job].id)), None);
                                    database_sender.send_prio(db_req);
                                    jobs.remove(job);
                                    scheduled_job = None;
                                },
                            }
                            current_deadline = schedule_job(&mut scheduled_job, database_sender.clone(), &jobs);
                        },
                        None => current_deadline = Utc::now().checked_add_days(Days::new(1)).unwrap(),
                    }
                }
            }
        }
    });
    
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Jobs {
    pub jobs:HashMap<JobID, Job>,
    pub latest_job_id:JobID
}

impl Jobs {
    pub fn new() -> Self {
        Self { jobs: HashMap::with_capacity(64), latest_job_id: 0 }
    }
    pub fn add_job(&mut self, mut job:Job) -> usize {
        let id = self.latest_job_id;
        self.latest_job_id += 1;
        job.id = id;
        self.jobs.insert(id, job);
        id
    }
    pub fn update_job(&mut self, job:Job) -> bool {
        self.jobs.insert(job.id, job).is_some()
    }
    pub fn remove_job(&mut self, job_id:JobID) {
        self.jobs.remove(&job_id);
    }
    pub fn get_job(&self, job_id:JobID) -> Option<&Job> {
        self.jobs.get(&job_id)
    }
}