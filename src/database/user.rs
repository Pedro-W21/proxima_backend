use std::f64;

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};

use crate::database::media::Base64EncodedString;

use super::{description::Description, tags::TagID};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersonalInformation {
    pub user_data:UserData,
    pub user_stats:UserStats
}

impl PersonalInformation {
    pub fn new(pseudonym:String, password:String) -> Self {
        let password_hash = data_into_base64_hash(password.as_bytes().to_vec());
        Self {
            user_data: UserData {last_updated:Utc::now(),password_hash,pseudonym:pseudonym.clone(), name:None, interests:Vec::with_capacity(100), current_description:Description::new(format!("The user is currently anonymous, called by the name : {}", pseudonym)) },
            user_stats:UserStats { heatmap:HeatMap::new(TimeDelta::minutes(5), TimeDelta::days(1), TimeDelta::days(7)) }
        }
    }
}

pub fn data_into_base64_hash(data:Vec<u8>) -> Base64EncodedString {
    let mut hasher = Sha3_256::new();
    hasher.update(&data);
    let hash:[u8 ; 32] = hasher.finalize().into();
    Base64EncodedString::new(hash.to_vec())
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserData {
    name:Option<String>,
    pub pseudonym:String,
    pub password_hash:Base64EncodedString,
    interests:Vec<Interest>,
    pub last_updated:DateTime<Utc>,
    current_description:Description
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserStats {
    pub heatmap:HeatMap, 
}



#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct HeatMap {
    slots:Vec<(f64, DateTime<Utc>)>,
    precision:TimeDelta,
    scale:TimeDelta,

    decay_rate:TimeDelta,
}

impl HeatMap {
    pub fn new(precision:TimeDelta, scale:TimeDelta, decay_rate:TimeDelta) -> Self {
        let slots = vec![(0.0, Utc::now()) ; scale.checked_div(precision.num_seconds() as i32).unwrap().num_seconds() as usize + 1];
        Self { slots, precision, scale, decay_rate }
    }
    pub fn get_at(&self, time:TimeDelta) -> Option<&(f64, DateTime<Utc>)> {
        let index = (time.num_seconds()/self.precision.num_seconds()) as usize;
        self.slots.get(index)
    }
    pub fn get_mut(&mut self, time:TimeDelta) -> Option<&mut (f64, DateTime<Utc>)> {
        let index = (time.num_seconds()/self.precision.num_seconds()) as usize;
        self.slots.get_mut(index)
    }
    pub fn increase_at(&mut self, time:TimeDelta, increase_time:DateTime<Utc>) {
        let decay_rate = self.decay_rate.clone();
        match self.get_mut(time) {
            Some((value, last_update)) => {
                let time_since_last_update = increase_time.signed_duration_since(last_update.clone());
                let power = time_since_last_update.num_seconds().abs()/decay_rate.num_seconds().abs();
                if power > 0 {  
                    *value = value.powf(0.9 * power as f64);
                    if *value <= 0.001 {
                        *value = 0.0;
                    }
                    *last_update = increase_time;
                }
                *value += 1.0;
            },
            None => ()
        }
    }
    pub fn update_all(&mut self, update_time:DateTime<Utc>) {
        let decay_rate = self.decay_rate.clone();
        for (value, last_update) in &mut self.slots {
            let time_since_last_update = update_time.signed_duration_since(last_update.clone());
            let power = time_since_last_update.num_seconds().abs()/decay_rate.num_seconds().abs();
            if power > 0 {  
                *value = value.powf(0.9 * power as f64);
                if *value <= 0.001 {
                    *value = 0.0;
                }
                *last_update = update_time;
            }
        }
    }
    pub fn get_first_lowest_slot(&mut self, from:TimeDelta) -> TimeDelta {
        self.update_all(Utc::now());

        let mut lowest = f64::INFINITY;
        for (slot, time) in &self.slots {
            if *slot <= lowest {
                lowest = *slot;
            }
        }
        let index = (from.num_seconds()/self.precision.num_seconds()) as usize;
        let mut first_lowest = index;
        for i in index..self.slots.len() {
            if self.slots[i].0 == lowest {
                first_lowest = i;
                break;
            }
        }
        if self.slots[first_lowest].0 != lowest {
            for i in 0..index {
                if self.slots[i].0 == lowest {
                    first_lowest = i;
                    break;
                }
            }
        }
        let first_lowest_time = self.precision.checked_mul(first_lowest as i32).unwrap();
        if from > first_lowest_time {
            self.scale.clone() + first_lowest_time
        }
        else {
            first_lowest_time
        }

    }

}

impl Eq for HeatMap {

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