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
