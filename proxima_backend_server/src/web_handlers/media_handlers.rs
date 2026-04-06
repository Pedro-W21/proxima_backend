use std::{io::ErrorKind, sync::Arc};

use actix_files::NamedFile;
use actix_web::{HttpRequest, HttpResponse, Responder, web};
use proxima_backend::{database::{DatabaseItem, DatabaseReply, DatabaseReplyVariant, DatabaseRequest, DatabaseRequestVariant, ToolRequest}, proxima_handler::ProximaHandler};



pub async fn media_get_handler(req: HttpRequest, data: web::Data<Arc<ProximaHandler>>) -> impl Responder {
    match req.full_url().path_segments().map(|path| {path.last()}) {
        Some(last_seg) => {
            let last = last_seg.unwrap();
            
            let (request, recv) = DatabaseRequest::new(DatabaseRequestVariant::ToolRequest(ToolRequest::GetMediaWithoutData(last.to_string())), None);
            data.database.send_prio(request);
            let reply = recv.recv();
            if let Ok(DatabaseReply {variant:DatabaseReplyVariant::ReturnedItem(DatabaseItem::Media(med, _))}) = reply {
                NamedFile::open(data.proxima_data_path.join(format!("media/{}", med.file_name)))
            }
            else {
                Err(std::io::Error::new(ErrorKind::InvalidFilename, ""))
            }
        },
        None => Err(std::io::Error::new(ErrorKind::InvalidFilename, ""))
    }
}