use crate::{ai_interaction::AiEndpointSender, database::DatabaseSender};

pub struct ProximaHandler {
    pub database:DatabaseSender,
    pub ai_endpoint:AiEndpointSender,
}