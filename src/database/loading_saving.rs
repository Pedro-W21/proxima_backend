use std::{collections::HashMap, fs::{DirBuilder, File}, io::Write, path::PathBuf, sync::LazyLock};

const PREMADE_FILES:LazyLock<HashMap<String, Vec<u8>>> = LazyLock::new(|| {
    HashMap::from(
        [
            ("description_prompt".to_string(), Vec::from(include_bytes!("../../configuration/prompts/description.txt"))),
            ("system_prompt".to_string(), Vec::from(include_bytes!("../../configuration/prompts/system.txt"))),
            ("internal_action_prompt".to_string(), Vec::from(include_bytes!("../../configuration/prompts/action.txt"))),

        ]
    )
});

const FOLDER_STRUCTURE:LazyLock<HashMap<String, PathBuf>> = LazyLock::new(|| {
    HashMap::from(
        [
            ("database".to_string(), PathBuf::from("personal_data/database/")),
            ("prompts".to_string(), PathBuf::from("configuration/prompts/")),
            ("description_prompt".to_string(), PathBuf::from("configuration/prompts/description.txt")),
            ("system_prompt".to_string(), PathBuf::from("configuration/prompts/system.txt")),
            ("internal_action_prompt".to_string(), PathBuf::from("configuration/prompts/action.txt")),
            ("user_folders_files".to_string(), PathBuf::from("personal_data/database/folders_and_files.json")),
            ("user_data".to_string(), PathBuf::from("personal_data/database/user_data.json")),
            ("tags".to_string(), PathBuf::from("personal_data/database/tags.json")),
            ("chats".to_string(), PathBuf::from("personal_data/database/chats.json")),

        ]
    )
    
});

pub fn create_or_repair_database_folder_structure(absolute_starting_folder:PathBuf) -> bool {
    let mut dir_builder = DirBuilder::new();
    let mut already_here = true;
    for (name, relative_path) in FOLDER_STRUCTURE.iter() {
        match relative_path.try_exists() {
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
                    already_here = false;
                    match File::create_new(absolute_starting_folder.join(relative_path.clone())) {
                        Ok(mut file_created) => {
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