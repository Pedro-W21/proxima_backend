use std::{fs::File, io::Read, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::database::{access_modes::AccessModeID, context::{ContextData, ContextPart, ContextPosition, Prompt, WholeContext}, description::DescriptionTarget, ProxDatabase};

#[derive(Clone, Serialize, Deserialize)]
pub struct AgentPrompt {
    access_mode:AccessModeID,
    prompt_type:AgentPromptType
}

#[derive(Clone, Serialize, Deserialize)]
pub enum AgentPromptType {
    Description{desc_target:DescriptionTarget},
    Tag{tag_target:WholeContext}
}

pub fn get_agent_prompt_context(database:&ProxDatabase,agent_prompt:AgentPrompt) -> WholeContext {
    match agent_prompt.prompt_type {
        AgentPromptType::Description { desc_target } => create_desc_prompt(database, desc_target, agent_prompt.access_mode),
        AgentPromptType::Tag { tag_target } => create_tag_prompt(database, tag_target, agent_prompt.access_mode)
    }
}

pub enum SystemPrompt {
    Description,
    Tag,
    Action,
}

pub fn create_desc_prompt(database:&ProxDatabase, desc_target:DescriptionTarget, access_mode:AccessModeID) -> WholeContext {
    let system = get_system_prompt_for(SystemPrompt::Description).unwrap();
    let target_type = format!("    target_type:{},\n", match desc_target {
        DescriptionTarget::File(id) => "file",
        DescriptionTarget::Folder(id) => "folder",
    });
    let target_data = match desc_target {
        DescriptionTarget::File(id) => {
            let file = database.files.get_file_by_id(id);
            format!("    target_data:{{\n        name:{},\n        path:{},\n        extension:{},\n        data:{}\n    }},", 
            file.get_name_string_lossy(),
            file.get_path().to_string_lossy().to_string(),
            file.get_extension_lossy().unwrap_or("null".to_string()),
            if file.is_pure_utf8() {file.get_pure_utf8()} else {"null".to_string()}
            )
        },
        DescriptionTarget::Folder(id) => {
            let folder = database.folders.get_folder_by_id(id);
            let mut folder_children = String::new();
            for folder_child in folder.get_folder_children() {
                let child = database.folders.get_folder_by_id(*folder_child);
                folder_children.push_str(format!("(\"folder\", {}, {}),", child.get_name_string(), match child.get_desc() {Some(desc) => desc.get_text().clone(), None => "null".to_string()} ).trim());
            }
            for file_child in folder.get_file_children() {
                let child = database.files.get_file_by_id(*file_child);
                folder_children.push_str(format!("(\"file\", {}, {}),", child.get_name_string_lossy(), match child.get_desc() {Some(desc) => desc.get_text().clone(), None => "null".to_string()} ).trim());
            }
            format!("    target_data:{{\n        name:{},\n        path:{},\n        children:{}\n    }},\n", 
            folder.get_name_string(),
            folder.get_full_path().to_string_lossy().to_string(),
            folder_children.trim_matches(',')
            )
        }
    };
    let existing_tags = {
        let all_tags = database.tags.get_tags().iter().enumerate();
        let mut list_elements = String::new();
        let access_mode_tags = database.access_modes.get_modes()[access_mode].get_tags();
        for (i, tag) in all_tags {
            if access_mode_tags.contains(&i) {
                list_elements.push_str(format!("({}, {}, {}),", tag.get_name(), tag.get_desc().get_text(), 
                    match tag.get_parent() {
                        Some(parent_id) => database.tags.get_tag_from_tagid(parent_id).unwrap().get_name(),
                        None => "null"
                    }
                ).as_str());
            }
            
        }
        list_elements = list_elements.trim_matches(',').to_string();
        format!("    existing_tags:[{}],\n", list_elements)
    };
    let user_description = format!("    user_description:\"{}\",\n", database.personal_info.user_data.get_desc().get_text().clone());
    let special = ContextPart::new(vec![ContextData::Text(format!("{{{}{}{}{}}}", target_type,target_data, existing_tags, user_description))], ContextPosition::User);
    WholeContext::new(vec![
        system,
        special
    ])
}

pub fn create_tag_prompt(database:&ProxDatabase, tagging_target:WholeContext, access_mode:AccessModeID) -> WholeContext {
    let system = get_system_prompt_for(SystemPrompt::Description).unwrap();
    let system_prompt = format!("system_prompt: \"{}\",", tagging_target.get_whole_system_prompt().concatenate_into_single_part().data_to_text().concat());
    let chat_data = format!("chat_data: \"{}\",", tagging_target.get_everything_but_system_prompt().concatenate_into_single_part().data_to_text().concat());
    let existing_tags = {
        let all_tags = database.tags.get_tags().iter().enumerate();
        let mut list_elements = String::new();
        let access_mode_tags = database.access_modes.get_modes()[access_mode].get_tags();
        for (i, tag) in all_tags {
            if access_mode_tags.contains(&i) {
                list_elements.push_str(format!("({}, {}, {}),", tag.get_name(), tag.get_desc().get_text(), 
                    match tag.get_parent() {
                        Some(parent_id) => database.tags.get_tag_from_tagid(parent_id).unwrap().get_name(),
                        None => "null"
                    }
                ).as_str());
            }
            
        }
        list_elements = list_elements.trim_matches(',').to_string();
        format!("    existing_tags:[{}],\n", list_elements)
    };
    let user_description = format!("    user_description:\"{}\",\n", database.personal_info.user_data.get_desc().get_text().clone());
    let special = ContextPart::new(vec![ContextData::Text(format!("{{{}{}{}{}}}", system_prompt,chat_data, existing_tags, user_description))], ContextPosition::User);
    WholeContext::new(vec![
        system,
        special
    ])
}

pub fn get_system_prompt_for(case:SystemPrompt) -> Result<Prompt, ()> {
    match case {
        SystemPrompt::Description => {
            open_prompt_file(PathBuf::from("configuration/prompts/description.txt"), ContextPosition::System)
        },
        SystemPrompt::Action => {
            open_prompt_file(PathBuf::from("configuration/prompts/action.txt"), ContextPosition::System)
        },
        SystemPrompt::Tag => {
            open_prompt_file(PathBuf::from("configuration/prompts/tag.txt"), ContextPosition::System)
        }
    }
}

pub fn open_prompt_file(file_path:PathBuf, position:ContextPosition) -> Result<Prompt, ()> {
    match File::open(file_path.clone()) {
        Ok(mut open_file) => {
            let mut string = String::with_capacity(1024);
            match open_file.read_to_string(&mut string) {
                Ok(bytes_read) => Ok(Prompt::new(vec![ContextData::Text(string)], position)),
                Err(error) => Err(())
            }
        },
        Err(error) => panic!("File that's supposed to exist doesn't {}", file_path.clone().to_string_lossy().to_string()),
    }
}