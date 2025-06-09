use serde::{Deserialize, Serialize};

use super::description::Description;

pub type TagID = usize;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tag {
    number:TagID,
    name:String,
    desc:Description,
    parent:Option<TagID>
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
    all_tags:Vec<Tag>
}

impl Tags {
    pub fn get_tags(&self) -> &Vec<Tag> {
        &self.all_tags
    }
    pub fn update_tag(&mut self, tag:Tag) {
        let num = tag.number;
        self.all_tags[num] = tag;
    }
    pub fn new() -> Self {
        Self { all_tags: Vec::with_capacity(256) }
    }
    pub fn add_tag_raw(&mut self, mut tag:Tag) -> TagID {
        let id = self.all_tags.len();
        tag.number = id;
        self.all_tags.push(tag);
        id
    }
    pub fn add_tag(&mut self, mut new_tag:NewTag) {
        match new_tag.parent {
            Some(tagid) => match self.get_tag_from_tagid(tagid) {
                Some(tag) => (),
                None => new_tag.parent = None,
            },
            None => ()
        }
        
        self.all_tags.push(Tag {number:self.all_tags.len(), name:new_tag.name, desc:new_tag.desc, parent:new_tag.parent})
    }
    pub fn add_tag_with_parent_name(&mut self, mut new_tag:NewTag, parent_name:Option<String>) {
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
        self.add_tag(new_tag);
    }
    pub fn get_tagid_of(&self, tag_name:String) -> Option<TagID> {
        self.all_tags.iter().enumerate().find(|(id, tag)| {tag.name == tag_name}).and_then(|(id, tag)| {Some(id)})
    }
    pub fn get_tag_from_tagid(&self, id:TagID) -> Option<&Tag> {
        self.all_tags.get(id)
    }
}