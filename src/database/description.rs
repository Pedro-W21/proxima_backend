use std::{str::FromStr, sync::LazyLock};

use html_parser::{Dom, Node};
use serde::{Deserialize, Serialize};

use super::{files::FileID, folders::FolderID, tags::{NewTag, TagID, Tags}};
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DescriptionTarget {
    File(FileID),
    Folder(FolderID),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Description {
    text:String,
}

impl Description {
    pub fn get_text(&self) -> &String {
        &self.text
    }
    pub fn new(text:String) -> Self {
        Self { text }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DescriptionParsingError {
    BadlyPlacedText,
    UnparseableDOM,
    IsCommented,
    BadNumberOfNewTagArgs,
    UnknownTag
}

pub fn parse_desc_response(text:String, tags:&mut Tags) -> Result<(Description, Vec<TagID>), DescriptionParsingError> {
    match Dom::parse(&text) {
        Ok(parsed) => {
            let mut recovered_errors = Vec::with_capacity(8);
            let mut desc = String::new();
            let mut used_tags = Vec::with_capacity(16);
            for child in parsed.children {
                match child {
                    Node::Element(elt) => {
                        match elt.name.trim() {
                            "Description" => {
                                desc = elt.children[0].text().unwrap().to_string().trim().to_string();
                            },
                            "NewTags" => {
                                for new_tag in elt.children[0].text().unwrap().trim().to_string().split("\n") {
                                    let parts:Vec<&str> = new_tag.split('|').collect();
                                    if parts.len() == 3 {
                                        let parts_0 = parts[0].trim().to_string();
                                        let name = parts_0.split(':').collect::<Vec<&str>>();
                                        let parts_1 = parts[1].trim().to_string();
                                        let desc = parts_1.split(':').collect::<Vec<&str>>();
                                        let parts_2 = parts[2].trim().to_string();
                                        let parent = parts_2.split(':').collect::<Vec<&str>>();
                                        
                                        tags.add_tag_with_parent_name(NewTag::new(if name.len() > 1 {name[1].to_string()} else {name[0].to_string()}, Description { text: if desc.len() > 1 {desc[1].to_string()} else {desc[0].to_string()}}, if if parent.len() == 2 {parent[1].to_string()} else {parent[0].to_string()} == "NONE" {None} else {tags.get_tagid_of(if parent.len() == 2 {parent[1].to_string()} else {parent[0].to_string()})}), if parent.len() == 2 {Some(parent[1].to_string())} else {Some(parent[0].to_string())});
                                    }
                                    else if parts.len() == 2 {
                                        let parts_0 = parts[0].trim().to_string();
                                        let name = parts_0.split(':').collect::<Vec<&str>>();
                                        let parts_1 = parts[1].trim().to_string();
                                        let desc = parts_1.split(':').collect::<Vec<&str>>();
                                        
                                        tags.add_tag(NewTag::new(if name.len() > 1 {name[1].to_string()} else {name[0].to_string()}, Description { text: if desc.len() > 1 {desc[1].to_string()} else {desc[0].to_string()}}, None));
                                    
                                    }
                                    else if parts.len() >= 3 {
                                        recovered_errors.push(DescriptionParsingError::BadNumberOfNewTagArgs);
                                        for part in parts {
                                            if part != "" {
                                                tags.add_tag(NewTag::new(part.trim().to_string(), Description { text:String::from("MISSING_DESCRIPTION") }, None));
                                            }
                                        }
                                    }
                                    else {
                                        return Err(DescriptionParsingError::BadNumberOfNewTagArgs)
                                    }
                                }
                            },
                            "Tagging" => {
                                for tag in elt.children[0].text().unwrap().to_string().split("\n") {
                                    match tags.get_tagid_of(tag.trim().to_string()) {
                                        Some(id) => used_tags.push(id),
                                        None => recovered_errors.push(DescriptionParsingError::UnknownTag),
                                    }
                                }
                            },
                            _ => ()
                        }
                    },
                    Node::Text(txt) => recovered_errors.push(DescriptionParsingError::BadlyPlacedText),
                    Node::Comment(com) => recovered_errors.push(DescriptionParsingError::IsCommented)
                }
            }
            Ok((Description{text:desc}, used_tags))
        },
        Err(_) => Err(DescriptionParsingError::UnparseableDOM)
    }
}