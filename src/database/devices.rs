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
    pub device_model:String
}

impl Device {
    pub fn new(id:DeviceID, device_name:String, device_type:DeviceType, device_os:String, device_model:String) -> Self {
        Self { id, device_name, device_type, device_os, device_model }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Devices {
    all_devices:Vec<Device>,
}

impl Devices {
    pub fn new() -> Self {
        Self { all_devices: Vec::with_capacity(32) }
    }
    pub fn get_devices(&self) -> &Vec<Device> {
        &self.all_devices
    }
    pub fn get_devices_mut(&mut self) -> &mut Vec<Device> {
        &mut self.all_devices
    }
    pub fn update_device(&mut self, mut device:Device) {
        let id = device.id;
        self.all_devices[id] = device;
        
    }
    pub fn add_device(&mut self, mut device:Device) -> DeviceID {
        let id = self.all_devices.len();
        device.id = id;
        self.all_devices.push(device);
        id
    }
}