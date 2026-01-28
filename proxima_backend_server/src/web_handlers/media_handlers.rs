use std::{io::ErrorKind, sync::Arc};

use actix_files::NamedFile;
use actix_web::{HttpRequest, HttpResponse, Responder, web};
use proxima_backend::proxima_handler::ProximaHandler;



pub async fn media_get_handler(req: HttpRequest, data: web::Data<Arc<ProximaHandler>>) -> impl Responder {
    match req.full_url().path_segments().map(|path| {path.last()}) {
        Some(last_seg) => {
            let last = last_seg.unwrap();
            
            NamedFile::open(data.proxima_data_path.join(format!("media/{}", last)))
        },
        None => Err(std::io::Error::new(ErrorKind::InvalidFilename, ""))
    }
}