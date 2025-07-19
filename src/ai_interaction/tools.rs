use std::collections::HashMap;

use html_parser::{Dom, Element, Node};
use serde::{Deserialize, Serialize};

use crate::database::{context::{ContextData, ContextPart, ContextPosition, WholeContext}, configuration::ChatSetting};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tools {
    used_tools:Vec<ProximaTool>,
    tool_data:HashMap<ProximaTool, ProximaToolData>
}

#[derive(Clone, Debug)]
pub enum ToolParsingError {
    BadElementName {expected:String, found:String},
    BadNumberOfArguments {expected:usize, found:usize, remarks:String},
    NotAnElement
}

impl Tools {
    pub fn try_from_settings(settings:Vec<ChatSetting>) -> Option<Self> {
        let mut used_tools = Vec::new();
        for setting in settings {
            match setting {
                ChatSetting::Tool(tool) => used_tools.push(tool),
                _ => ()
            }
        }
        if used_tools.len() > 0 {
            let mut tool_data = HashMap::new();
            for tool in &used_tools {
                match tool.get_empty_data() {
                    Some(empty_data) => tool_data.insert(tool.clone(), empty_data),
                    None => None
                };
            }
            Some(Tools { used_tools, tool_data })
        }
        else {
            None
        }
    }
    pub fn get_tool_data_insert(&self) -> ContextPart {
        let mut part = ContextPart::new(vec![], ContextPosition::Tool);
        for (key, data) in &self.tool_data {
            part.add_data(data.get_data_to_insert());
        }
        part
    }
    pub fn get_tool_calling_sys_prompt(&self) -> ContextPart {
        let mut base = String::from_utf8(Vec::from(include_bytes!("../../configuration/prompts/tool_prompts/tool_use.txt"))).unwrap();
        for tool in &self.used_tools {
            base += &tool.get_description_string();
        }
        base += &String::from("\n</ToolUse>");
        ContextPart::new(vec![ContextData::Text(base)], ContextPosition::System)
    }
    pub fn call(&self, call_element:Element) -> Result<(ContextData, Self), ContextPart> {
        if call_element.children.len() == 3 {
            let mut tool_name = String::new();
            match &call_element.children[0] {
                Node::Element(tool_element) => match tool_element.name.trim() {
                    "tool" => match tool_element.children.get(0).map(|node| {node.text().unwrap_or("NOT A TOOL")}) {
                        Some(name) => tool_name = String::from(name),
                        None => ()
                    },
                    other => return Err(ProximaToolCallError::Parsing(ToolParsingError::BadElementName { expected: String::from("tool"), found: String::from(other) }).generate_error_output("Couldn't be parsed".to_string(), "Couldn't be parsed".to_string()))
                },
                _ => return Err(ProximaToolCallError::Parsing(ToolParsingError::NotAnElement).generate_error_output("Couldn't be parsed".to_string(), "Couldn't be parsed".to_string()))
            }
            if let Some(tool) = ProximaTool::try_from_string(tool_name.clone()) {
                let mut action = String::new();
                match &call_element.children[1] {
                    Node::Element(tool_element) => match tool_element.name.trim() {
                        "action" => match tool_element.children.get(0).map(|node| {node.text().unwrap_or("NOT AN ACTION")}) {
                            Some(name) => action = String::from(name),
                            None => ()
                        },
                        other => return Err(ProximaToolCallError::Parsing(ToolParsingError::BadElementName { expected: String::from("action"), found: String::from(other) }).generate_error_output(tool_name, "Couldn't be parsed".to_string()))
                    },
                    _ => return Err(ProximaToolCallError::Parsing(ToolParsingError::NotAnElement).generate_error_output(tool_name, "Couldn't be parsed".to_string()))
                }
                if tool.is_valid_action(&action) {
                    let mut inputs = String::new();
                    match &call_element.children[2] {
                        Node::Element(tool_element) => match tool_element.name.trim() {
                            "inputs" => match tool_element.children.get(0).map(|node| {node.text().unwrap_or("NOT AN INPUT")}) {
                                Some(name) => inputs = String::from(name),
                                None => ()
                            },
                            other => return Err(ProximaToolCallError::Parsing(ToolParsingError::BadElementName { expected: String::from("input"), found: String::from(other) }).generate_error_output(tool_name, action))
                        },
                        _ => return Err(ProximaToolCallError::Parsing(ToolParsingError::NotAnElement).generate_error_output(tool_name, action))
                    }
                    return tool.respond_to(action.clone(), inputs, self.tool_data.get(&tool)).map(|(context, new_data)| {(context, 
                    match new_data {
                        Some(new_data) => {
                            let mut new_self = self.clone();
                            new_self.tool_data.insert(tool.clone(), new_data);
                            new_self
                        },
                        None => self.clone()
                    })}).map_err(|error| {error.generate_error_output(tool_name, action)})
                }
            }

            return Err(ProximaToolCallError::Parsing(ToolParsingError::NotAnElement).generate_error_output("Couldn't be parsed".to_string(), "Couldn't be parsed".to_string()));
        }
        else {
            Err(ProximaToolCallError::Parsing(ToolParsingError::NotAnElement).generate_error_output("Couldn't be parsed".to_string(), "Couldn't be parsed".to_string()))
        }
    }
}

#[derive(Clone, Debug)]
pub enum ProximaToolCallError {
    Parsing(ToolParsingError),

}

impl ProximaToolCallError {
    pub fn generate_error_output(&self, tool:String, action:String) -> ContextPart {
        ContextPart::new(vec![self.generate_error_output_just_context_data(tool, action)], ContextPosition::Tool)  
    }
    pub fn generate_error_output_just_context_data(&self, tool:String, action:String) -> ContextData{
        ContextData::Text(format!("<error><tool>{tool}</tool><action>{action}</action><error_data>{:?}</error_data></error>", self))
    }
}

pub fn generate_call_output(tool:String, action:String, output_data:String) -> ContextData {
    ContextData::Text(format!("<output><tool>{tool}</tool><action>{action}</action><data>{output_data}</data></output>"))
}

#[derive(Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProximaTool {
    LocalMemory,
    Calculator
}

impl ProximaTool {
    pub fn must_insert_data(&self) -> bool {
        match self {
            Self::LocalMemory => true,
            Self::Calculator => false,
        }
    }
    pub fn is_valid_action(&self, action:&String) -> bool {
        match self {
            Self::LocalMemory => match action.trim() {
                "add" | "update" | "remove" => true,
                _ => false
            },
            Self::Calculator => todo!("Implement calculator")
        }
    }
    pub fn try_from_string(string:String) -> Option<Self> {
        match string.trim() {
            "Local Memory" => Some(Self::LocalMemory),
            "Calculator" => Some(Self::Calculator),
            _ => None
        }
    }
    pub fn respond_to(&self, action:String, input:String, data:Option<&ProximaToolData>) -> Result<(ContextData, Option<ProximaToolData>), ProximaToolCallError> {
        match self {
            Self::LocalMemory => {
                let mut new_data = data.unwrap().get_local_mem_data();
                let input_lines:Vec<String> = input.trim().lines().map(|line| {line.trim().to_string()}).collect();
                match action.trim() {
                    "add" => if input_lines.len() >= 2 {
                        let key = input_lines[0].clone();
                        let value = input_lines[1..].iter().intersperse(&String::from("\n")).collect::<Vec<&String>>().iter().map(|string| {(*string).clone()}).collect::<Vec<String>>().concat();
                        new_data.insert(key, value);
                        Ok((generate_call_output("Local Memory".to_string(), "add".to_string(), "".to_string()), Some(ProximaToolData::LocalMemory(new_data))))
                    }
                    else {
                        Err(ProximaToolCallError::Parsing(ToolParsingError::BadNumberOfArguments { expected: 2, found: input_lines.len(), remarks: format!("The first input line contains the key, all the rest contain the value assigned to that key") }))
                    },
                    "update" => if input_lines.len() >= 2 {
                        let key = input_lines[0].clone();
                        let value = input_lines[1..].iter().intersperse(&String::from("\n")).collect::<Vec<&String>>().iter().map(|string| {(*string).clone()}).collect::<Vec<String>>().concat();
                        new_data.insert(key, value);
                        Ok((generate_call_output("Local Memory".to_string(), "update".to_string(), "".to_string()), Some(ProximaToolData::LocalMemory(new_data))))
                    }
                    else {
                        Err(ProximaToolCallError::Parsing(ToolParsingError::BadNumberOfArguments { expected: 2, found: input_lines.len(), remarks: format!("The first input line contains the key, all the rest contain the value assigned to that key") }))
                    },
                    "remove" => if input_lines.len() == 1 {
                        let key = input_lines[0].clone();
                        new_data.remove(&key);
                        Ok((generate_call_output("Local Memory".to_string(), "remove".to_string(), "".to_string()), Some(ProximaToolData::LocalMemory(new_data))))
                    }
                    else {
                        Err(ProximaToolCallError::Parsing(ToolParsingError::BadNumberOfArguments { expected: 1, found: input_lines.len(), remarks: format!("The first input line contains the key, there are no other lines") }))
                    },
                    _ => panic!("Impossible, action must be checked before this point")
                }
            },
            Self::Calculator => todo!("Implement calculator")
        }
    }
    pub fn get_empty_data(&self) -> Option<ProximaToolData> {
        match self {
            Self::LocalMemory => Some(ProximaToolData::LocalMemory(HashMap::new())),
            Self::Calculator => None
        }
    }
    pub fn get_description_string(&self) -> String {
        match self {
            Self::LocalMemory => String::from(include_str!("../../configuration/prompts/tool_prompts/local_memory.txt")),
            Self::Calculator => todo!("Implement calculator")
        }
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProximaToolData {
    LocalMemory(HashMap<String, String>)
}

impl ProximaToolData {
    pub fn get_data_to_insert(&self) -> ContextData {
        match self {
            Self::LocalMemory(key_value) => ContextData::Text(format!("<LocalMemory> local memory data : {:?}<LocalMemory>", key_value.clone()))
        }
    }
    pub fn get_local_mem_data(&self) -> HashMap<String, String> {
        match self {
            Self::LocalMemory(key_value) => key_value.clone()
        }
    }
}


pub fn handle_tool_calling_response(response:ContextPart, tools:Tools) -> (ContextPart, Tools) {
    let mut out_context = ContextPart::new(vec![ContextData::Text(format!("<outputs>\n"))], ContextPosition::Tool);
    let mut out_tools = tools.clone();
    for data in response.get_data() {
        match data {
            ContextData::Text(text) => {
                let (part, part_tools) = handle_tool_calling_context_data(text, out_tools.clone());
                out_tools = part_tools;
                out_context.merge_data_with(part);
            },
            _ => ()
        }
    }
    out_context.add_data(ContextData::Text(format!("\n</outputs>")));
    (out_context, out_tools)
}

pub fn is_valid_tool_calling_response(response:&ContextPart) -> bool {
    let mut found_start = false;
    let mut found_end = false;
    let mut found_call = false;
    for data in response.get_data() {
        match data {
            ContextData::Text(text) => {
                found_call = found_call || text.contains("<call>");
                if found_start {
                    found_end = found_end || text.contains("</response>");
                }
                else {
                    found_start = found_start || text.contains("<response>");
                    found_end = found_end || text.contains("</response>");
                }
            },
            _ => ()
        }
    }
    found_start && found_end && !found_call
}

fn handle_tool_calling_context_data(text:&String, tools:Tools) -> (ContextPart, Tools) {
    match Dom::parse(text) {
        Ok(parsed) => {
            for child in parsed.children {
                match child {
                    Node::Element(elt) => {
                        match elt.name.trim() {
                            "call" => {
                                match tools.call(elt) {
                                    Ok((context_data, out_tools)) => return (ContextPart::new(vec![context_data], ContextPosition::Tool), out_tools),
                                    Err(error) => return (error, tools),
                                }
                            },
                            _ => ()
                        }
                    },
                    _ => ()
                }
            }
            (ContextPart::new(vec![], ContextPosition::Tool), tools)
        },
        Err(_) => (ContextPart::new(vec![], ContextPosition::Tool), tools)
    }
}