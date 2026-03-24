use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub type DeviceID = usize;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeviceType {
    Smartphone,
    Desktop,
    Laptop,
    SmartGlasses,
    SmartWatch,
    Other(String)
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Device {
    pub id:DeviceID,
    pub device_name:String,
    pub device_type:DeviceType,
    pub device_os:String,
    pub device_model:String,
    pub added_on:DateTime<Utc>
}

impl Device {
    pub fn new(id:DeviceID, device_name:String, device_type:DeviceType, device_os:String, device_model:String) -> Self {
        Self { id, device_name, device_type, device_os, device_model, added_on:Utc::now() }
    }
    pub fn get_id(&self) -> DeviceID {
        self.id
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Devices {
    all_devices:HashMap<DeviceID, Device>,
    pub latest_id:DeviceID,
}

impl Devices {
    pub fn new() -> Self {
        Self { all_devices: HashMap::from([(0, Device::new(0, 
            
            whoami::devicename(),
            match std::env::consts::OS {
                "android" | "ios" => DeviceType::Smartphone,
                _ => DeviceType::Laptop
            },
            std::env::consts::OS.into(),
            String::from("Generic computing device (I don't know man)"),
        ))]), latest_id:1}
    }
    pub fn get_devices(&self) -> &HashMap<DeviceID, Device> {
        &self.all_devices
    }
    pub fn get_devices_mut(&mut self) -> &mut HashMap<DeviceID, Device> {
        &mut self.all_devices
    }
    pub fn update_device(&mut self, mut device:Device) -> bool {
        let id = device.id;
        self.all_devices.insert(id, device).is_some()
    }
    pub fn add_device(&mut self, mut device:Device) -> DeviceID {
        let id = self.latest_id;
        self.latest_id += 1;
        device.id = id;
        self.all_devices.insert(id, device);
        id
    }
}