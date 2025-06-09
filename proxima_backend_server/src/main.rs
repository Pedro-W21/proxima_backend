#![feature(mpmc_channel)]

use std::{path::PathBuf, sync::{mpmc::channel, Arc}};

use actix_web::{web::Data, App, HttpServer};
use proxima_backend::ai_interaction::{launch_ai_endpoint_thread};
use proxima_backend::database::launch_database_thread;
use proxima_backend::initialization::initialize;
use proxima_backend::proxima_handler::ProximaHandler;
use openai::Credentials;
use actix_web::web;
use web_handlers::{ai_endpoint_web_handlers::ai_post_handler, auth_web_handlers::auth_post_handler, database_web_handlers::db_post_handler, home_endpoint_web_handlers::home_get_handler};
use openai_impl::{ChosenModel, OpenAIBackend};

use futures::{join, try_join};

pub mod web_handlers;
pub mod openai_impl;

#[actix_web::main]
async fn main() {
    let initialization_data = initialize();
    let database = proxima_backend::database::ProxDatabase::new(String::from("aaa"), String::from("aaa"), PathBuf::from("/home/pir/ia/proxima_testing_grounds"));
    let database_sender = launch_database_thread(database);
    let p1 = channel();
    let p2 = channel();
    let (endpoint_sender, handle) = launch_ai_endpoint_thread::<OpenAIBackend>((Credentials::new("SDQKJHSFKL","http://localhost:5001/v1/"), ChosenModel::from("RARA")), database_sender.clone(), p1.0, p1.1, p2.0, p2.1).await;
    let handler = Arc::new(ProximaHandler {ai_endpoint:endpoint_sender, database:database_sender});
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(handler.clone())) // Share the handler
            .route("/home", web::get().to(home_get_handler))
            .route("/auth", web::post().to(auth_post_handler))
            .route("/db", web::post().to(db_post_handler))
            .route("/ai", web::post().to(ai_post_handler))
    })
    .bind("127.0.0.1:8082")
    .unwrap()
    .run();
    join!(server, handle.join().unwrap());
    println!("WHAAT");
}
