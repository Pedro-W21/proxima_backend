use std::collections::HashMap;

use chrono::{DateTime, Utc};
use html_parser::{Dom, Node};
use serde::{Deserialize, Serialize};

use super::description::Description;

pub type TagID = usize;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tag {
    number:TagID,
    pub name:String,
    pub desc:Description,
    pub created_at:DateTime<Utc>,
    pub parent:Option<TagID>
}

impl Tag {
    pub fn get_name(&self) -> &String {
        &self.name
    }
    pub fn get_desc(&self) -> &Description {
        &self.desc
    }
    pub fn get_parent(&self) -> Option<TagID> {
        self.parent
    }
    pub fn get_id(&self) -> TagID {
        self.number
    }
    pub fn set_id(&mut self, id:TagID) {
        self.number = id;
    }
    
    pub fn new(number:TagID, name:String, desc:Description, parent:Option<TagID>) -> Self {
        Self { number, name, desc, parent, created_at:Utc::now() }
    }
}

pub struct NewTag {
    name:String,
    desc:Description,
    parent:Option<TagID>
}

impl NewTag {
    pub fn new(name:String, desc:Description, parent:Option<TagID>) -> Self {
        Self { name, desc, parent}
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tags {
    all_tags:HashMap<TagID,Tag>,
    last_id:usize,
}

impl Tags {
    pub fn get_tags(&self) -> &HashMap<TagID, Tag> {
        &self.all_tags
    }
    pub fn get_tags_mut(&mut self) -> &mut HashMap<TagID, Tag> {
        &mut self.all_tags
    }
    pub fn update_tag(&mut self, new_tag:Tag) {
        let num = new_tag.number;
        self.all_tags.insert(num, new_tag);
    }
    pub fn new() -> Self {
        Self { all_tags: HashMap::with_capacity(256),last_id:0 }
    }
    pub fn add_tag_raw(&mut self, mut tag:Tag) -> TagID {
        let id = self.last_id;
        tag.number = id;
        self.all_tags.insert(id, tag);
        self.last_id += 1;
        id
    }
    pub fn add_tag(&mut self, mut new_tag:NewTag) -> TagID {
        match new_tag.parent {
            Some(tagid) => match self.get_tag_from_tagid(tagid) {
                Some(tag) => (),
                None => new_tag.parent = None,
            },
            None => ()
        }
        let tag_id = self.last_id;
        self.all_tags.insert(tag_id,Tag {number:tag_id, name:new_tag.name, desc:new_tag.desc, parent:new_tag.parent, created_at:Utc::now()});
        self.last_id += 1;
        tag_id
    }
    pub fn create_possible_tag(&self, mut new_tag:NewTag) -> Tag {
        match new_tag.parent {
            Some(tagid) => match self.get_tag_from_tagid(tagid) {
                Some(tag) => (),
                None => new_tag.parent = None,
            },
            None => ()
        }
        let tag = Tag {number:self.last_id, name:new_tag.name, desc:new_tag.desc, parent:new_tag.parent, created_at:Utc::now()};
        tag
    }
    pub fn add_tag_with_parent_name(&mut self, mut new_tag:NewTag, parent_name:Option<String>) -> TagID {
        match parent_name {
            Some(parent_tag) => match self.get_tagid_of(parent_tag.clone()) {
                Some(tagid) => (),
                None => {
                    let tagid = self.all_tags.len();
                    new_tag.parent = Some(tagid);
                    self.add_tag(NewTag { name: parent_tag, desc: Description::new(String::from("Missing description")), parent:None });
                }
            },
            None => ()
        }
        self.add_tag(new_tag)
    }
    pub fn get_tagid_of(&self, tag_name:String) -> Option<TagID> {
        self.all_tags.iter().find(|(id, tag)| {tag.name == tag_name}).and_then(|(id, tag)| {Some(*id)})
    }
    pub fn get_tag_from_tagid(&self, id:TagID) -> Option<&Tag> {
        self.all_tags.get(&id)
    }
    pub fn get_last_tag(&self) -> Option<&Tag> {
        self.all_tags.get(&self.last_id.checked_sub(1).unwrap_or(0))
    }
}




#[derive(Clone, Copy, Debug)]
pub enum TaggingParsingError {
    BadlyPlacedText,
    UnparseableDOM,
    IsCommented,
    BadNumberOfNewTagArgs,
    UnknownTag
}

pub fn parse_tagging_response(text:String, tags:&mut Tags) -> Result<Vec<TagID>, TaggingParsingError> {
    match Dom::parse(&text) {
        Ok(parsed) => {
            let mut recovered_errors = Vec::with_capacity(8);
            let mut used_tags = Vec::with_capacity(16);
            for child in parsed.children {
                match child {
                    Node::Element(elt) => {
                        match elt.name.trim() {
                            "Tagging" => {
                                for tag in elt.children[0].text().unwrap().to_string().split("\n") {
                                    match tags.get_tagid_of(tag.trim().to_string()) {
                                        Some(id) => used_tags.push(id),
                                        None => recovered_errors.push(TaggingParsingError::UnknownTag),
                                    }
                                }
                            },
                            _ => ()
                        }
                    },
                    Node::Text(txt) => recovered_errors.push(TaggingParsingError::BadlyPlacedText),
                    Node::Comment(com) => recovered_errors.push(TaggingParsingError::IsCommented)
                }
            }
            Ok(used_tags)
        },
        Err(_) => Err(TaggingParsingError::UnparseableDOM)
    }
}