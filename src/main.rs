#![feature(proc_macro_hygiene, decl_macro)]
#![feature(seek_stream_len)]
#![feature(let_else)]

#[allow(unused_imports)]
#[allow(unused_macros)]
/* Logging crates */
#[macro_use]
extern crate log;

/* Parallelism and multithreading crates */
extern crate futures;
extern crate rayon;
extern crate tokio;

/* random number generation */
extern crate rand;

/* http support crates */
#[macro_use]
extern crate rocket;

extern crate mime;

/* compression crates */
//extern crate flate2;
//extern crate tar;

/* crypt crates */
extern crate data_encoding;
//extern crate ring;

/* database crates */
//extern crate rusqlite;

/* filesystem utility crates */
extern crate notify;

/* general utility crates */
//extern crate chrono;
extern crate lazy_static;
extern crate regex;

mod filecache;
mod routes;

use std::{fs, path::PathBuf};

use filecache::*;
use lazy_static::lazy_static;
use rocket::fairing::AdHoc;

static CACHE_DIR: &str = "CACHE/";

lazy_static! {
    static ref FILECACHE: FileCache = {
        let mut cwd = std::env::current_dir().unwrap();
        cwd.push(CACHE_DIR);

        FileCache::new(&cwd)
    };
}

lazy_static! {
    static ref CACHE_PATH: PathBuf = {
        let mut cwd = std::env::current_dir().unwrap();
        cwd.push(&CACHE_DIR);

        cwd
    };
}

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    let _rocket = rocket::build()
        .mount(
            "/",
            rocket::routes![
                routes::index,
                routes::download_file,
                routes::query_file,
                routes::upload_file
            ],
        )
        .attach(AdHoc::on_liftoff("Application setup", |_| {
            Box::pin(async move {
                lazy_static::initialize(&CACHE_PATH);
                lazy_static::initialize(&FILECACHE);

                let keep_file = {
                    let mut marker_path = PathBuf::from(&CACHE_PATH.as_path());
                    marker_path.push("rfile.meta");

                    marker_path
                };

                if !(&CACHE_PATH.is_dir()) {
                    match fs::create_dir(&CACHE_PATH.as_path()) {
                        Ok(_) => log::info!(
                            "crated new cache directory @ {}",
                            &CACHE_PATH.as_path().to_string_lossy()
                        ),
                        Err(e) => {
                            log::error!("Error creating cache directory - {}", &e);
                            panic!("{}", &e);
                        }
                    }
                }

                if !keep_file.exists() {
                    match fs::File::create(&keep_file) {
                        Ok(_) => log::info!(
                            "crated new enviroment file @ {}",
                            &CACHE_PATH.as_path().to_string_lossy()
                        ),
                        Err(e) => {
                            log::error!("Error creating enviroment file - {}", &e);
                            panic!("{}", &e);
                        }
                    }
                }

                match FILECACHE.add(&keep_file) {
                    Ok(_) => {
                        log::info!("added {} to cache", &keep_file.as_path().to_string_lossy())
                    }
                    Err(e) => match e {
                        CacheEntryError::NotFound => todo!(),
                        CacheEntryError::FileLocked => todo!(),
                        CacheEntryError::FileExists => todo!(),
                    },
                }
            })
        }))
        .ignite().await?
        .launch().await?;

        Ok(())
    }
