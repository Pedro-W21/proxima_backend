use std::{collections::HashMap, fs::{DirBuilder, File}, io::{Read, Write}, path::PathBuf, sync::LazyLock};

use serde::{de::DeserializeOwned, Deserialize};

use crate::database::{access_modes::AccessModes, chats::Chats, devices::Devices, files::Files, folders::Folders, tags::Tags, user::PersonalInformation, ProxDatabase};

const PREMADE_FILES:LazyLock<HashMap<String, Vec<u8>>> = LazyLock::new(|| {
    HashMap::from(
        [
            ("description_prompt".to_string(), Vec::from(include_bytes!("../../configuration/prompts/description.txt"))),
            ("system_prompt".to_string(), Vec::from(include_bytes!("../../configuration/prompts/system.txt"))),
            ("internal_action_prompt".to_string(), Vec::from(include_bytes!("../../configuration/prompts/action.txt"))),
            ("local_memory_prompt".to_string(), Vec::from(include_bytes!("../../configuration/prompts/tool_prompts/local_memory.txt"))),
            ("folders".to_string(), serde_json::to_string(&Folders::new()).unwrap().as_bytes().to_vec()),
            ("files".to_string(), serde_json::to_string(&Files::new()).unwrap().as_bytes().to_vec()),
            ("chats".to_string(), serde_json::to_string(&Chats::new()).unwrap().as_bytes().to_vec()),
            ("devices".to_string(), serde_json::to_string(&Devices::new()).unwrap().as_bytes().to_vec()),
            ("access_modes".to_string(), serde_json::to_string(&AccessModes::new()).unwrap().as_bytes().to_vec()),
            ("tags".to_string(), serde_json::to_string(&Tags::new()).unwrap().as_bytes().to_vec()),
            ("user_data".to_string(), serde_json::to_string(&PersonalInformation::new(String::new(), String::new())).unwrap().as_bytes().to_vec()),
        ]
    )
});

const FOLDER_STRUCTURE:LazyLock<HashMap<String, PathBuf>> = LazyLock::new(|| {
    HashMap::from(
        [
            ("database".to_string(), PathBuf::from("personal_data/database/")),
            ("prompts".to_string(), PathBuf::from("configuration/prompts/")),
            ("tool_prompts".to_string(), PathBuf::from("configuration/prompts/tool_prompts")),
            ("description_prompt".to_string(), PathBuf::from("configuration/prompts/description.txt")),
            ("system_prompt".to_string(), PathBuf::from("configuration/prompts/system.txt")),
            ("internal_action_prompt".to_string(), PathBuf::from("configuration/prompts/action.txt")),
            ("folders".to_string(), PathBuf::from("personal_data/database/folders.json")),
            ("files".to_string(), PathBuf::from("personal_data/database/files.json")),
            ("user_data".to_string(), PathBuf::from("personal_data/database/user_data.json")),
            ("tags".to_string(), PathBuf::from("personal_data/database/tags.json")),
            ("chats".to_string(), PathBuf::from("personal_data/database/chats.json")),
            ("access_modes".to_string(), PathBuf::from("personal_data/database/access_modes.json")),
            ("devices".to_string(), PathBuf::from("personal_data/database/devices.json")),

        ]
    )
    
});

pub fn create_or_repair_database_folder_structure(absolute_starting_folder:PathBuf) -> bool {
    let mut dir_builder = DirBuilder::new();
    let mut already_here = true;
    for (name, relative_path) in FOLDER_STRUCTURE.iter() {
        match absolute_starting_folder.join(relative_path).try_exists() {
            Ok(confirmation) => if !confirmation {
                already_here = false;
                if relative_path.extension().is_none() {
                    dir_builder.recursive(true).create(absolute_starting_folder.join(relative_path)).unwrap();
                }
            },
            Err(error) => panic!("Problem creating folders")
        }
    }
    for (name, relative_path) in FOLDER_STRUCTURE.iter() {
        match relative_path.try_exists() {
            Ok(confirmation) => if !confirmation {
                if relative_path.file_name().is_some() && relative_path.extension().is_some() {
                    match File::create_new(absolute_starting_folder.join(relative_path.clone())) {
                        Ok(mut file_created) => {
                            already_here = false;
                            println!("File {} created", relative_path.clone().to_string_lossy().to_string());
                            match PREMADE_FILES.get(name) {
                                Some(data) => {
                                    println!("Default data encoded for this file");
                                    file_created.write_all(data).unwrap();
                                },
                                None => ()
                            } 
                        },
                        Err(error) => println!("Couldn't create file {} because of {}", relative_path.to_string_lossy().to_string(), error)
                    }
                }
            },
            Err(error) => panic!("Problem creating folders")
        }
    }
    already_here
}

fn save_string_into_file(string:String, file:PathBuf) -> Result<(), std::io::Error> {
    match File::create(file) {
        Ok(mut file_created) => file_created.write_all(&string.as_bytes()),
        Err(error) => Err(error)
    }
}

pub fn save_to_disk(database:ProxDatabase, absolute_starting_folder:PathBuf) -> Result<(),std::io::Error> {
    let string = serde_json::to_string(&database.folders).unwrap();
    save_string_into_file(string, absolute_starting_folder.join(FOLDER_STRUCTURE.get("folders").unwrap()))?;

    let string = serde_json::to_string(&database.files).unwrap();
    save_string_into_file(string, absolute_starting_folder.join(FOLDER_STRUCTURE.get("files").unwrap()))?;

    let string = serde_json::to_string(&database.devices).unwrap();
    save_string_into_file(string, absolute_starting_folder.join(FOLDER_STRUCTURE.get("devices").unwrap()))?;

    let string = serde_json::to_string(&database.access_modes).unwrap();
    save_string_into_file(string, absolute_starting_folder.join(FOLDER_STRUCTURE.get("access_modes").unwrap()))?;

    let string = serde_json::to_string(&database.chats).unwrap();
    save_string_into_file(string, absolute_starting_folder.join(FOLDER_STRUCTURE.get("chats").unwrap()))?;

    let string = serde_json::to_string(&database.tags).unwrap();
    save_string_into_file(string, absolute_starting_folder.join(FOLDER_STRUCTURE.get("tags").unwrap()))?;

    let string = serde_json::to_string(&database.personal_info).unwrap();
    save_string_into_file(string, absolute_starting_folder.join(FOLDER_STRUCTURE.get("user_data").unwrap()))?;

    Ok(())
}

fn load_json_from_file<T:DeserializeOwned>(file:PathBuf) -> Result<T, serde_json::Error> {
    let mut string = String::new();
    match File::open(file) {
        Ok(mut file_opened) => file_opened.read_to_string(&mut string) ,
        Err(error) => Err(error)
    };
    let value = serde_json::from_str(&string);
    value
}

pub fn load_from_disk(absolute_starting_folder:PathBuf) -> Result<ProxDatabase, serde_json::Error> {
    let folders = load_json_from_file::<Folders>(absolute_starting_folder.join(FOLDER_STRUCTURE.get("folders").unwrap()))?;
    let files = load_json_from_file::<Files>(absolute_starting_folder.join(FOLDER_STRUCTURE.get("files").unwrap()))?;
    let tags = load_json_from_file::<Tags>(absolute_starting_folder.join(FOLDER_STRUCTURE.get("tags").unwrap()))?;
    let devices = load_json_from_file::<Devices>(absolute_starting_folder.join(FOLDER_STRUCTURE.get("devices").unwrap()))?;
    let access_modes = load_json_from_file::<AccessModes>(absolute_starting_folder.join(FOLDER_STRUCTURE.get("access_modes").unwrap()))?;
    let chats = load_json_from_file::<Chats>(absolute_starting_folder.join(FOLDER_STRUCTURE.get("chats").unwrap()))?;
    let personal_information = load_json_from_file::<PersonalInformation>(absolute_starting_folder.join(FOLDER_STRUCTURE.get("user_data").unwrap()))?;
    Ok(ProxDatabase::from_parts(files, folders, chats, tags, personal_information, absolute_starting_folder, devices, access_modes))
}