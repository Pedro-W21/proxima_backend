use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{description::Description, tags::TagID};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersonalInformation {
    pub user_data:UserData,
}

impl PersonalInformation {
    pub fn new(pseudonym:String, password_hash:String) -> Self {
        Self { user_data: UserData {last_updated:Utc::now(),password_hash,pseudonym:pseudonym.clone(), name:None, interests:Vec::with_capacity(100), current_description:Description::new(format!("The user is currently anonymous, called by the name : {}", pseudonym)) } }
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserData {
    name:Option<String>,
    pub pseudonym:String,
    pub password_hash:String,
    interests:Vec<Interest>,
    pub last_updated:DateTime<Utc>,
    current_description:Description
}

impl UserData {
    pub fn get_desc(&self) -> Description {
        self.current_description.clone()
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Interest {
    tags:Vec<TagID>,
    description:Description
}