use std::{cmp::Ordering, collections::HashMap};

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
    NotAnElement,
    IncorrectExpression {expression:String, issue:String}
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
    pub async fn call(&self, call_element:Element) -> Result<(ContextData, Self), ContextPart> {
        dbg!(call_element.clone());
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
                _ => return Err(ProximaToolCallError::Parsing(ToolParsingError::NotAnElement).generate_error_output("Tool couldn't be parsed".to_string(), "Couldn't be parsed".to_string()))
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
                    _ => return Err(ProximaToolCallError::Parsing(ToolParsingError::NotAnElement).generate_error_output(tool_name, "Action couldn't be parsed".to_string()))
                }
                if tool.is_valid_action(&action) {
                    let mut inputs = String::new();
                    match &call_element.children[2] {
                        Node::Element(tool_element) => match tool_element.name.trim() {
                            "in_data" => match tool_element.children.get(0).map(|node| {node.text().unwrap_or("NOT AN INPUT")}) {
                                Some(name) => inputs = String::from(name),
                                None => ()
                            },
                            other => return Err(ProximaToolCallError::Parsing(ToolParsingError::BadElementName { expected: String::from("input"), found: String::from(other) }).generate_error_output(tool_name, action))
                        },
                        _ => return Err(ProximaToolCallError::Parsing(ToolParsingError::NotAnElement).generate_error_output(tool_name, action))
                    }
                    return tool.respond_to(action.clone(), inputs, self.tool_data.get(&tool)).await.map(|(context, new_data)| {(context, 
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

            return Err(ProximaToolCallError::Parsing(ToolParsingError::NotAnElement).generate_error_output("Tool name couldn't be parsed".to_string(), "Couldn't be parsed".to_string()));
        }
        else {
            Err(ProximaToolCallError::Parsing(ToolParsingError::NotAnElement).generate_error_output("Couldn't be parsed".to_string(), "Couldn't be parsed".to_string()))
        }
    }
}

#[derive(Clone, Debug)]
pub enum ProximaToolCallError {
    Parsing(ToolParsingError),
    WebError(String)
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
    ContextData::Text(format!("<output><tool>{tool}</tool><action>{action}</action><output_data>{output_data}</output_data></output>"))
}

#[derive(Clone, Hash, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum ProximaTool {
    LocalMemory,
    Calculator,
    Web
}

impl ProximaTool {
    pub fn must_insert_data(&self) -> bool {
        match self {
            Self::LocalMemory => true,
            Self::Calculator => false,
            Self::Web => false
        }
    }
    pub fn is_valid_action(&self, action:&String) -> bool {
        match self {
            Self::LocalMemory => match action.trim() {
                "add" | "update" | "remove" => true,
                _ => false
            },
            Self::Calculator => match action.trim() {
                "compute" | "check" => true,
                _ => false
            },
            Self::Web => match action.trim() {
                "search" | "open" => true,
                _ => false
            },
        }
    }
    pub fn try_from_string(string:String) -> Option<Self> {
        match string.trim() {
            "Local Memory" => Some(Self::LocalMemory),
            "Calculator" => Some(Self::Calculator),
            "Web" => Some(Self::Web),
            _ => None
        }
    }
    pub async fn respond_to(&self, action:String, input:String, data:Option<&ProximaToolData>) -> Result<(ContextData, Option<ProximaToolData>), ProximaToolCallError> {
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
            Self::Calculator => {
                let input_lines:Vec<String> = input.trim().lines().map(|line| {line.trim().to_string()}).collect();
                match action.trim() {
                    "compute" => if input_lines.len() >= 1 {
                        let mut output = String::new();
                        for line in input_lines {
                            match string_calculator::eval_f64(line.clone(), 1.0) {
                                Ok(value) => output += format!("{} = {:.4}\n", line, value).trim(),
                                Err(error) => return Err(ProximaToolCallError::Parsing(ToolParsingError::IncorrectExpression { expression: line, issue: error.to_string() }))
                            }
                        }
                        Ok((generate_call_output("Calculator".to_string(), "compute".to_string(), output), None))
                    }
                    else {
                        Err(ProximaToolCallError::Parsing(ToolParsingError::BadNumberOfArguments { expected: 1, found: 0, remarks: format!("You must provide at least 1 expression to compute") }))
                    },
                    "check" => if input_lines.len() >= 1 {
                        const COMPARATORS:[(char, Ordering) ; 3] = [('>', Ordering::Greater), ('<', Ordering::Less), ('=', Ordering::Equal)];
                        let mut output = String::new();
                        'lines:for line in input_lines {
                            for (comparator,ordering) in &COMPARATORS {
                                if line.contains(*comparator) {
                                    let exprs:Vec<String> = line.split(*comparator).map(|expr| {expr.trim().to_string()}).collect();
                                    if exprs.len() == 2 {
                                        let val1 = match string_calculator::eval_f64(exprs[0].clone(), 1.0) {
                                            Ok(value) => value,
                                            Err(error) => return Err(ProximaToolCallError::Parsing(ToolParsingError::IncorrectExpression { expression: exprs[0].to_string(), issue: error.to_string() }))
                                        };
                                        let val2 = match string_calculator::eval_f64(exprs[1].clone(), 1.0) {
                                            Ok(value) => value,
                                            Err(error) => return Err(ProximaToolCallError::Parsing(ToolParsingError::IncorrectExpression { expression: exprs[1].to_string(), issue: error.to_string() }))
                                        };
                                        if val1.total_cmp(&val2) == *ordering {
                                            output += format!("{} -> TRUE\n", line).trim()
                                        }
                                        else {
                                            output += format!("{} -> FALSE\n", line).trim()
                                        }
                                    }
                                    else {
                                        return Err(ProximaToolCallError::Parsing(ToolParsingError::BadNumberOfArguments { expected: 2, found: exprs.len(), remarks: format!("You must provide only 2 expressions to compare") }))
                                    }
                                    continue 'lines;
                                }
                            }
                            
                        }
                        Ok((generate_call_output("Calculator".to_string(), "check".to_string(), output), None))
                    }
                    else {
                        Err(ProximaToolCallError::Parsing(ToolParsingError::BadNumberOfArguments { expected: 1, found: 0, remarks: format!("You must provide at least 1 line of expressions to check") }))
                    },
                    _ => panic!("Impossible, action must be checked before this point")
                }
            },
            Self::Web => {

                let input_lines:Vec<String> = input.trim().lines().map(|line| {line.trim().to_string()}).collect();
                match action.trim() {
                    "search" => if input_lines.len() >= 1 {
                        let mut output = String::new();
                        for line in input_lines {
                            let mut words:Vec<&str> = line.split_whitespace().collect();
                            if words.len() >= 2 {

                                match words[0].parse::<usize>() {
                                    Ok(value) => if !cfg!(target_family = "wasm") {
                                        words.remove(0);
                                        match web_search_tool(value, words.into_iter().intersperse(&" ").collect::<Vec<&str>>().concat().trim().trim_matches('"').to_string()).await {
                                            Ok(addition) => {
                                                output += &format!("Query: {}\n#####\n", line.clone());
                                                output += &addition;    
                                            },
                                            Err(error) => return Err(error)
                                        }
                                    },
                                    Err(_) => return Err(ProximaToolCallError::Parsing(ToolParsingError::BadNumberOfArguments { expected: 1, found: 0, remarks: format!("The first argument on each line must be ") }))
                                }
                            }
                            else {
                                return Err(ProximaToolCallError::Parsing(ToolParsingError::BadNumberOfArguments { expected: 2, found: 1, remarks: format!("a query has 2 arguments, the number of results and the text of the query itself") }))
                            }
                        }
                        Ok((generate_call_output("Web".to_string(), "search".to_string(), output), None))
                    }
                    else {
                        Err(ProximaToolCallError::Parsing(ToolParsingError::BadNumberOfArguments { expected: 1, found: 0, remarks: format!("You must provide at least 1 query to search") }))
                    },
                    "open" => if input_lines.len() >= 1 {
                        let mut output = String::new();
                        if !cfg!(target_family = "wasm") {

                            match web_open_tool(input_lines).await {
                                Ok(out) => output = out,
                                Err(error) => return Err(error)
                            }
                        }   
                        Ok((generate_call_output("Web".to_string(), "open".to_string(), output), None))
                    }
                    else {
                        Err(ProximaToolCallError::Parsing(ToolParsingError::BadNumberOfArguments { expected: 1, found: 0, remarks: format!("You must provide at least 1 website to open") }))
                    },
                    _ => panic!("Impossible, action must be checked before this point")
                }
            }
        }
    }
    pub fn get_empty_data(&self) -> Option<ProximaToolData> {
        match self {
            Self::LocalMemory => Some(ProximaToolData::LocalMemory(HashMap::new())),
            Self::Calculator => None,
            Self::Web => None,
        }
    }
    pub fn get_description_string(&self) -> String {
        match self {
            Self::LocalMemory => String::from(include_str!("../../configuration/prompts/tool_prompts/local_memory.txt")),
            Self::Calculator => String::from(include_str!("../../configuration/prompts/tool_prompts/calculator.txt")),
            Self::Web => String::from(include_str!("../../configuration/prompts/tool_prompts/web.txt")),
        }
    }
    pub fn get_name(&self) -> String {
        match self {
            Self::Calculator => format!("Calculator"),
            Self::LocalMemory => format!("Local memory"),
            Self::Web => format!("Web")
        }
    }
}

#[cfg(not(target_family = "wasm"))]
async fn web_search_tool(number_of_results:usize, query:String) -> Result<String, ProximaToolCallError> {
    use duckduckgo::browser::Browser;
    use duckduckgo::user_agents::get;
    use reqwest::Client;
    let mut output = String::new();
    let browser = Browser::new(Client::new());
    match browser.lite_search(&query, "wt-wt", Some(number_of_results), get("firefox").unwrap()).await {
        Ok(results) => for result in results {
            output += &format!("Title: {}\nURL: {}\nSnippet: {}\n-----------------\n", result.title, result.url, result.snippet);
        },
        Err(error) => return Err(ProximaToolCallError::WebError(format!("{}", error)))
    }
    Ok(output)
}


#[cfg(all(target_family = "wasm"))]
async fn web_search_tool(number_of_results:usize, query:String) -> Result<String, ProximaToolCallError> {
    Err(ProximaToolCallError::WebError(format!("Running a web search tool call on a WASM platform, not supported")))
}

#[cfg(not(target_family = "wasm"))]
async fn web_open_tool(lines:Vec<String>) -> Result<String, ProximaToolCallError> {
    use reqwest::Client;
    let mut output = String::new();
    let client = Client::new();
    for url in lines {
        match client.get(url.clone()).header("User-Agent", "ProximaBotWebTool/0.1 (https://github.com/Pedro-W21/proxima_backend) reqwest/0.11.27").send().await {
            Ok(response) => match response.error_for_status() {
                Ok(real_res) => output += &format!("{} : ```{}```\n", url, real_res.text().await.unwrap()),
                Err(error) => return Err(ProximaToolCallError::WebError(format!("{}", error)))
            },
            Err(error) => return Err(ProximaToolCallError::WebError(format!("{}", error)))
        }
    }
    Ok(output)
}

#[cfg(all(target_family = "wasm"))]
async fn web_open_tool(lines:Vec<String>) -> Result<String, ProximaToolCallError> {
    Err(ProximaToolCallError::WebError(format!("Running a web open tool call on a WASM platform, not supported")))
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


pub async fn handle_tool_calling_response(response:ContextPart, tools:Tools) -> (ContextPart, Tools) {
    let mut out_context = ContextPart::new(vec![ContextData::Text(format!("<outputs>\n"))], ContextPosition::Tool);
    let mut out_tools = tools.clone();
    for data in response.get_data() {
        match data {
            ContextData::Text(text) => {
                let (part, part_tools) = handle_tool_calling_context_data(text, out_tools.clone()).await;
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

async fn handle_tool_calling_context_data(text:&String, mut tools:Tools) -> (ContextPart, Tools) {
    match Dom::parse(text) {
        Ok(parsed) => {
            let mut data = Vec::with_capacity(2);
            for child in parsed.children {
                match child {
                    Node::Element(elt) => {
                        match elt.name.trim() {
                            "call" => {
                                match tools.call(elt).await {
                                    Ok((context_data, out_tools)) => {
                                        data.push(context_data);
                                        tools = out_tools;
                                    },
                                    Err(error) => return (error, tools),
                                }
                            },
                            _ => ()
                        }
                    },
                    _ => ()
                }
            }
            (ContextPart::new(data, ContextPosition::Tool), tools)
        },
        Err(_) => (ContextPart::new(vec![], ContextPosition::Tool), tools)
    }
}