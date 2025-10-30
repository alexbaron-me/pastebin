#[macro_use]
extern crate rocket;

mod paste_id;
#[cfg(test)]
mod tests;

use std::io;

use rocket::data::{Data, ToByteUnit};
use rocket::http::uri::Absolute;
use rocket::response::content::{RawHtml, RawText};
use rocket::tokio::fs::{self, File};

use paste_id::PasteId;

const ID_LENGTH: usize = 3;

fn host() -> Absolute<'static> {
    let raw = std::env::var("HOST").unwrap_or_else(|_| "http://localhost:8000".to_owned());
    Absolute::parse_owned(raw).expect("Received invalid HOST parameter")
}

#[post("/", data = "<paste>")]
async fn upload(paste: Data<'_>) -> io::Result<String> {
    let id = PasteId::new(ID_LENGTH);
    paste
        .open(128.kibibytes())
        .into_file(id.file_path())
        .await?;
    Ok(uri!(host(), retrieve(id)).to_string())
}

#[get("/upload")]
async fn upload_ui() -> RawHtml<&'static str> {
    RawHtml(
        "
<!DOCTYPE html>
<html>
    <body>
        <form action='/' method='post'>
            <input type='file' />
            <button type='submit'>Upload</button>
        </form>
    </body>
</html>
    ",
    )
}

#[get("/<id>")]
async fn retrieve(id: PasteId<'_>) -> Option<RawText<File>> {
    File::open(id.file_path()).await.map(RawText).ok()
}

#[delete("/<id>")]
async fn delete(id: PasteId<'_>) -> Option<()> {
    fs::remove_file(id.file_path()).await.ok()
}

#[get("/")]
fn index() -> String {
    format!(
        "
    USAGE

      POST /

          accepts raw data in the body of the request and responds with a URL of
          a page containing the body's content

          EXAMPLE: curl --data-binary @file.txt {0}

      GET /<id>

          retrieves the content for the paste with id `<id>`

    UPLOAD VIA BROWSER
      
      GET {0}/upload

          provides a simple upload UI
    ",
        host(),
    )
}

#[launch]
fn rocket() -> _ {
    let upload_path = PasteId::file_root_dir();
    if !std::fs::exists(&upload_path).unwrap_or(false) {
        let _ = std::fs::create_dir(&upload_path);
    }

    rocket::build().mount("/", routes![index, upload, upload_ui, delete, retrieve])
}
