use std::{fs::File, io, path::PathBuf};

pub struct FileLock(PathBuf);

impl FileLock {
    pub fn new(path: PathBuf) -> Result<Self, io::Error> {
        File::create_new(&path).map(|_| Self(path))
    }
}

impl std::ops::Drop for FileLock {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).ok();
    }
}
