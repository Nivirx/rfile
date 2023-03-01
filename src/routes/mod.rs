use std::path::PathBuf;

use rocket::{Response, Request, response};
use rocket::form::Form;
use rocket::fs::{NamedFile, TempFile};
use rocket::http::{ContentType, Status};
use rocket::response::{content, Debug};
use rocket::response::Responder;

static UPLOAD_HTML: &str = include_str!("../html/upload.html");

#[get("/")]
pub async fn index() -> Result<content::RawHtml<&'static str>, Debug<std::io::Error>> {
    let response = content::RawHtml(UPLOAD_HTML);
    Ok(response)
}

// TODO: This should probably be moved to FileCache and a responder made for FileEntries
// currently we get a ref to a RwLock<FileEntry> that we lock for read access and grab the path
// to pass to a new NamedFile which is then wrapped in a CachedFile<T>
pub struct CachedFile(NamedFile);

// Custom responder for the wrapper CachedFile, uses the response the NamedFile would return
// and sets Content-Disposition(file type and filename) and Cache-control
impl<'r> Responder<'r, 'static> for CachedFile {
    fn respond_to(self, req: &'r Request) -> response::Result<'static> {

        let name = match self.0.path().file_name() {
            Some(f) => { 
                f.to_string_lossy().to_string()
        },
            None => todo!()
        };

        Response::build_from(self.0.respond_to(req)?)
            .raw_header("Content-Disposition", format!("application/octet-stream; filename=\"{}\"", name))
            .raw_header("Cache-control", "max-age=86400") //  24h (24*60*60)
            .ok()
    }
}

#[get("/download/<file_hash>")]
pub async fn download_file(file_hash: String) -> Option<CachedFile> {
    let file = match crate::FILECACHE.get(file_hash) {
        Ok(fe) => fe,
        Err(_) => {
            panic!()
        }
    };

    let file = {
        let path_lock = file.read().unwrap().open_path();
        let path = path_lock.read().unwrap().clone();

        match NamedFile::open(path).await.ok() {
            Some(f) => f,
            None => todo!()
        }
    };

    log::debug!("{:#?}", NamedFile::path(&file));
    Some(CachedFile(file))
}

#[get("/search/<query>")]
pub async fn query_file(query: String) -> String {
    // TODO: implement some sort of search function...maybe...
    format!("[NOT IMPLEMENTED] Looking up {}...", &query)
}

// FIXME: Clippy suggestion (Issue is in rocket's implementation of #[derive(FromForm)] )
// warning: unnecessary closure used with `bool::then`
//     --> src/routes/mod.rs:71:11
//    |
// 71 |     data: TempFile<'f>,
//    |           ^^^^^^^^^^^^ help: use `then_some(..)` instead: `then_some(data)`
//    |
//    = note: `#[warn(clippy::unnecessary_lazy_evaluations)]` on by default
//    = help: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#unnecessary_lazy_evaluations

#[derive(FromForm)]
pub struct UploadFile<'f> {
    data: TempFile<'f>,
}

#[post("/", data = "<form>")]
pub async fn upload_file(mut form: Form<UploadFile<'_>>) -> Result<String, String> {
    //this is how a POST looks from our web form (simplified)
    //Content-Disposition: form-data; name="filename"; filename="1510287646273.png"
    //Content-Type: image/png
    // <left blank>
    //---DATA---

    let filename = match form.data.name() {
        Some(s) => String::from(s),
        None => return Err(Status::BadRequest.to_string()),
    };

    let raw_filename = match form.data.raw_name() {
        Some(s) => <&rocket::fs::FileName>::clone(&s),
        None => return Err(Status::BadRequest.to_string()),
    };

    let filetype = match form.data.content_type() {
        Some(ct) => ct.clone(),
        None => ContentType::Plain,
    };

    let extension = match filetype.extension() {
        Some(ext) => ext.to_string(),
        None => {
            let mut name = String::new();
            let unsafe_name = raw_filename.dangerous_unsafe_unsanitized_raw();

            if unsafe_name.len() < 255 {
                name = match unsafe_name.as_str().trim().split_once('.') {
                    Some((_, ext)) => {
                        let mut ext = ext.replace("..", "").replace('/', "").replace(':', "");

                        if ext.starts_with('.') {
                            ext = ext[1..].to_string();
                        }

                        ext
                    }
                    None => "badext".to_string(),
                };
            }
            log::debug!(
                "unhandled extension uploaded...parse attempt ended up with {}",
                &name
            );

            name
        }
    };

    let filepath = PathBuf::from(format!(
        "{}{}.{}",
        &crate::CACHE_PATH.as_path().display(),
        &filename,
        extension
    ));
    let copy_res = match form.data.copy_to(filepath.as_path()).await {
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    };
    
    let hasher = crate::FileEntry::new(filepath.as_path()).get_hash_string();

    match (copy_res, hasher) {
        (Ok(_), h) => Ok(format!(
            "OK...received {}.{} size:{} bytes\nBLAKE3 Hash: {}",
            &filename,
            &extension,
            form.data.len(),
            &h
        )),
        (Err(_), _) => Err(Status::InternalServerError.to_string()),
    }
}
