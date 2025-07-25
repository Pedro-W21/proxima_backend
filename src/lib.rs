#![feature(mpmc_channel)]
#![feature(step_trait)]
#![feature(iter_intersperse)]

use std::{path::PathBuf, sync::{mpmc::channel, Arc}};

use ai_interaction::{launch_ai_endpoint_thread};
use database::launch_database_thread;
use initialization::initialize;
use proxima_handler::ProximaHandler;
pub mod database;
pub mod ai_interaction;
pub mod proxima_handler;
pub mod initialization;
pub mod web_payloads;

async fn initialize_server() {
    let initialization_data = initialize();
    
}
