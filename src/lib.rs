#![feature(mpmc_channel)]

use std::{path::PathBuf, sync::{mpmc::channel, Arc}};

use ai_interaction::{launch_ai_endpoint_thread};
use database::launch_database_thread;
use initialization::initialize;
use proxima_handler::ProximaHandler;
use openai::Credentials;
pub mod database;
pub mod ai_interaction;
pub mod proxima_handler;
pub mod initialization;

async fn initialize_server() {
    let initialization_data = initialize();
    
}
