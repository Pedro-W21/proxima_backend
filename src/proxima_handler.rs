use std::path::PathBuf;

use crate::{ai_interaction::AiEndpointSender, database::DatabaseSender};

pub struct ProximaHandler {
    pub database:DatabaseSender,
    pub proxima_data_path:PathBuf,
    pub ai_endpoint:AiEndpointSender,
}