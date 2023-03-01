#![allow(dead_code)]

use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::io::{Cursor, ErrorKind};
use std::sync::{
    mpsc::{channel, Receiver},
    Arc, RwLock,
};

use std::thread;
use std::time::Duration;

use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};

type CacheGuard<T> = Arc<RwLock<T>>;

#[derive(Debug)]
pub enum FileEntryError {
    NeedsUpdate,
    EmptyFile,
    FileOpenError,
}

#[derive(Debug)]
pub struct FileEntry {
    path: CacheGuard<std::path::PathBuf>,
    hash: CacheGuard<Option<blake3::Hash>>,
    _content: CacheGuard<Option<std::io::BufReader<File>>>,
}

impl FileEntry {
    pub fn new(file_path: &std::path::Path) -> FileEntry {
        let mut fe = FileEntry {
            path: Arc::new(RwLock::new(file_path.to_path_buf())),
            hash: Arc::new(RwLock::new(None)),
            _content: Arc::new(RwLock::new(None)),
        };

        fe.do_hash();
        log::trace!("{:#?}", fe);

        fe
    }

    pub fn get_hash_string(&self) -> String {
        let hash_ptr = self.hash.read().unwrap();
        hash_ptr.unwrap().to_string()
    }

    pub fn open_path(&self) -> CacheGuard<String> {
        let ptr = self.path.clone();
        let path = ptr.read().unwrap();
        let path = path.as_path().to_string_lossy();

        Arc::new(RwLock::new(String::from(&*path)))
    }

    // HACK: this whole function has been hacked up trying to fix dead locks.
    fn do_hash(&mut self) {
        let path_ptr = self.path.clone();
        let path_lock = path_ptr.read().unwrap();

        if path_lock.is_file() {
            let path_lock = self.path.clone();
            let file = std::fs::File::open(path_lock.read().unwrap().as_path());
            let file = file.unwrap_or_else(|_| panic!("issue opening file for {:#?}", &self.path));

            let temp_content = unsafe {
                match memmap2::Mmap::map(&file) {
                    Ok(mm) => Cursor::new(mm),
                    Err(_) => panic!(),
                }
            };

            let mut hasher = blake3::Hasher::new();
            // HACK: this feels bad...
            hasher.update_rayon(
                &temp_content
                    .bytes()
                    // FIXME: clippy suggestion below
                    // warning: `filter(..).map(..)` can be simplified as `filter_map(..)`
                    // --> src/filecache/mod.rs:82:22
                    //   |
                    //82 |                       .filter(|b| b.is_ok())
                    //   |  ______________________^
                    //83 | |                     .map(|b| b.unwrap())
                    //   | |________________________________________^ help: try: `filter_map(|b| b.ok())`
                    //   |
                    //   = note: `#[warn(clippy::manual_filter_map)]` on by default
                    //   = help: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#manual_filter_map

                    .filter(|b| b.is_ok())
                    .map(|b| b.unwrap())
                    .collect::<Vec<_>>(),
            );

            *self.hash.write().unwrap() = Some(hasher.finalize());
        }
        /*

        loop {
            let has_content = ptr.read().unwrap().is_some();
            if has_content {
                let mut hasher = blake3::Hasher::new();
                hasher.update_rayon(content.get_ref());


                let hash_ptr = self.hash.clone();
                let hash_ptr = hash_ptr.write().unwrap();
                let hash_ptr = Some(hasher.finalize());

                break;
            } else {

                let content_lock = self.content.clone();
                let mut content = content_lock.write().unwrap();

                if content.is_none() {
                    let path_lock = self.path.clone();
                    let file = std::fs::File::open(path_lock.read().unwrap().as_path());
                    let file = file.unwrap_or_else(|_| panic!("issue opening file for {:#?}", &self.path));


                continue;
            }
        }

         */
    }
}

#[derive(Debug)]
pub enum CacheEntryError {
    NotFound,
    FileLocked,
    FileExists,
}

pub struct FileCache {
    cache: CacheGuard<HashMap<String, CacheGuard<FileEntry>>>,
    cache_dir: std::path::PathBuf,
    notify_watcher: RecommendedWatcher,
    notify_thread: std::thread::JoinHandle<()>,
}

impl FileCache {
    pub fn new(cache_dir: &std::path::Path) -> FileCache {
        let (tx, rx) = channel();

        match std::fs::create_dir(&cache_dir) {
            Ok(_) => log::info!(
                "Created new file directory @ {}",
                cache_dir.to_string_lossy()
            ),
            Err(e) => match e.kind() {
                ErrorKind::AlreadyExists => {
                    log::info!("Attempted to create a new dir: {} for FileCache, but it already exists (This is normal)", cache_dir.to_string_lossy());
                }
                ErrorKind::PermissionDenied => {
                    log::error!("Attempted to create a new dir: {} for FileCache, but permission was denined\n{}", cache_dir.to_string_lossy(), &e)
                }
                _ => {
                    log::error!("{}", &e)
                }
            },
        }

        let mut fc = FileCache {
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_dir: cache_dir.to_path_buf(),
            notify_watcher: notify::Watcher::new(tx, Duration::from_secs(1)).unwrap(),
            notify_thread: thread::Builder::new()
                .name("notify-thread".to_string())
                .spawn(move || FileCache::notify_loop(rx))
                .unwrap(),
        };
        fc.notify_watcher
            .watch(&fc.cache_dir, RecursiveMode::Recursive)
            .unwrap();

        fc
    }

    pub fn get(&self, file_hash: String) -> Result<CacheGuard<FileEntry>, CacheEntryError> {
        let cache_lock = Arc::clone(&self.cache);
        let cache_lock = cache_lock.read().unwrap();

        // FIXME: clippy suggestion below
        // warning: temporary with significant `Drop` in `match` scrutinee will live until the end of the `match` expression
        //    --> src/filecache/mod.rs:178:15
        //        |
        //    178 |   match cache_lock.get(&file_hash) {
        //        |         ^^^^^^^^^^^^^^^^^^^^^^^^^^
        //    ... 
        //    181 |   }
        //        |   - temporary lives until here
        //        |
        //        = note: `#[warn(clippy::significant_drop_in_scrutinee)]` on by default
        //        = note: this might lead to deadlocks or other unexpected behavior
        //        = help: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#significant_drop_in_scrutinee

        match cache_lock.get(&file_hash) {
            Some(fe) => Ok(fe.clone()),
            None => Err(CacheEntryError::NotFound),
        }
    }

    pub fn add(&self, file_path: &std::path::Path) -> Result<String, CacheEntryError> {
        let fe = FileEntry::new(file_path);
        let hash = fe.get_hash_string();

        let cache_lock = self.cache.clone();
        let mut cache_lock = cache_lock.write().unwrap();

        cache_lock
            .entry(fe.get_hash_string())
            .or_insert_with(|| Arc::new(RwLock::new(fe)));

        Ok(hash)
    }

    fn notify_loop(rx: Receiver<DebouncedEvent>) {
        let this_thread = std::thread::current();
        log::info!("notify loop starting on thread-{:?}", &this_thread);

        let thread_name: String = match &this_thread.name() {
            Some(s) => s.to_string(),
            None => "notify-thread".to_string(),
        };

        loop {
            match rx.recv() {
                Ok(event) => {
                    log::info!("[{}] {:?}", thread_name, &event);

                    // FIXME: need to cover all event types with appropiate actions and ignore the rest.
                    match event {
                        DebouncedEvent::NoticeWrite(_) => {
                            // do nothing, this is sent when a file is being updated
                        }
                        DebouncedEvent::NoticeRemove(_) => {
                            // do nothing, this is sent when a file is being removed
                        }
                        DebouncedEvent::Create(p) => {
                            match crate::FILECACHE.add(p.as_path()) {
                                Ok(hash) => log::info!("[{}] Found new file @ {} ({})",thread_name, &p.as_path().to_string_lossy(), hash.to_string()),
                                Err(e) => log::error!("[{}] Found a new file but there was an error adding to internal cache\nFile -> {} \nError -> {:#?}", thread_name, &p.as_path().to_string_lossy(), &e),
                            }
                        }
                        DebouncedEvent::Write(p) => {
                            match crate::FILECACHE.add(p.as_path()) {
                                Ok(hash) => log::info!("[{}] A file was updated @ {} ({})",thread_name, &p.as_path().to_string_lossy(), hash.to_string()),
                                Err(e) => log::error!("[{}] Found a new file but there was an error updating the internal cache\nFile -> {} \nError -> {:#?}", thread_name, &p.as_path().to_string_lossy(), &e),
                            }
                        }
                        DebouncedEvent::Chmod(_) => {},
                        DebouncedEvent::Remove(_) => {}
                        DebouncedEvent::Rename(_, _) => {}
                        DebouncedEvent::Rescan => {},
                        DebouncedEvent::Error(_, _) => log::debug!(
                            "received a Error event on watched dir, but we don't do anything!"
                        ),
                    }
                }
                Err(e) => log::info!("watch error: {:?}", e),
            }
        }
    }
}
