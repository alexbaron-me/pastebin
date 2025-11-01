#[macro_use]
extern crate rocket;

mod paste_id;
#[cfg(test)]
mod tests;

use std::io;
use std::str::FromStr;

use mime_sniffer::MimeTypeSniffer;
use rocket::data::{Data, ToByteUnit};
use rocket::form::Form;
use rocket::fs::TempFile;
use rocket::http::ContentType;
use rocket::http::uri::Absolute;
use rocket::response::content::RawHtml;
use rocket::tokio::fs::{self, File};

use paste_id::PasteId;
use rocket::tokio::io::AsyncReadExt;
use rocket_prometheus::PrometheusMetrics;
use rocket_prometheus::prometheus::{Gauge, PullingGauge};

const ID_LENGTH: usize = 4;

pub(crate) fn host() -> Absolute<'static> {
    let raw = std::env::var("HOST").unwrap_or_else(|_| "http://localhost:8000".to_owned());
    Absolute::parse_owned(raw).expect("Received invalid HOST parameter")
}

#[post("/", data = "<paste>")]
async fn upload(paste: Data<'_>) -> io::Result<PasteId<'_>> {
    let id = PasteId::new(ID_LENGTH);
    paste
        .open(128.kibibytes())
        .into_file(id.file_path())
        .await?;
    Ok(id)
}

#[derive(FromForm)]
struct FileUpload<'a> {
    file: TempFile<'a>,
}

#[post("/upload", data = "<form>")]
async fn upload_ui_handler(mut form: Form<FileUpload<'_>>) -> io::Result<PasteId<'_>> {
    let id = PasteId::new(ID_LENGTH);
    form.file.move_copy_to(id.file_path()).await?;
    Ok(id)
}

#[get("/upload")]
async fn upload_ui() -> RawHtml<&'static str> {
    RawHtml(
        "
<!DOCTYPE html>
<html>
    <body>
        <form method='post' enctype='multipart/form-data'>
            <input type='file' name='file' id='file' />
            <button type='submit'>Upload</button>
        </form>
    </body>
</html>
    ",
    )
}

#[get("/<id>")]
pub(crate) async fn retrieve(id: PasteId<'_>) -> Option<(ContentType, Vec<u8>)> {
    let mut file = File::open(id.file_path()).await.ok()?;
    let mut content = Vec::new();
    let _ = file.read_to_end(&mut content).await;

    let mime = content.sniff_mime_type();
    let content_type = mime
        .map(ContentType::from_str)
        .map(Result::ok)
        .flatten()
        .unwrap_or(ContentType::Text);

    Some((content_type, content))
}

#[delete("/<id>")]
async fn delete(id: PasteId<'_>) -> Option<()> {
    fs::remove_file(id.file_path()).await.ok()
}

#[get("/")]
fn index() -> RawHtml<String> {
    RawHtml(format!(
        "<html><pre>
    USAGE

      POST /

          accepts raw data in the body of the request and responds with a URL of
          a page containing the body's content

          EXAMPLE: curl --data-binary @file.txt {0}

      GET /&lt;id&gt;

          retrieves the content for the paste with id `&lt;id&gt;`

    UPLOAD VIA BROWSER
      
      GET <a href='{0}/upload'>{0}/upload</a>

          provides a simple upload UI
    </pre></html>",
        host(),
    ))
}

fn stored_files() -> f64 {
    let dir = PasteId::file_root_dir();
    std::fs::read_dir(dir)
        .map(|res| res.count() as f64)
        .unwrap_or(-1f64)
}

#[launch]
fn rocket() -> _ {
    let upload_path = PasteId::file_root_dir();
    if !std::fs::exists(&upload_path).unwrap_or(false) {
        let _ = std::fs::create_dir(&upload_path);
    }

    let prometheus = PrometheusMetrics::new();
    prometheus
        .registry()
        .register(Box::new(
            PullingGauge::new(
                "pastebin_stored_files",
                "Amount of files currently stored in the pastebin",
                Box::new(|| stored_files()),
            )
            .expect("Failed to create pastebin_stored_files openmetrics gauge"),
        ))
        .expect("Failed to add custom prometheus metric");

    rocket::build()
        .attach(prometheus.clone())
        .mount(
            "/",
            routes![
                index,
                upload,
                upload_ui,
                upload_ui_handler,
                delete,
                retrieve,
            ],
        )
        .mount("/metrics", prometheus)
}
