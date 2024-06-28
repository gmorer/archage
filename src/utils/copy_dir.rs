use std::{
    fs::{copy, create_dir_all, read_dir, read_link},
    io,
    os::unix::fs,
    path::{Path, PathBuf},
};

// fn create_dir_all(path: &Path) -> Result<(), io::Error> {
//     if path.exists() {
//         return Ok(());
//     }
//     if let Some(parent) = path.parent() {
//         if parent.exists() {
//             create_dir(path)?;
//         } else {
//             create_dir_all(parent)?;
//         }
//     }
//     Ok(())
// }

// Copy a dir recursilvy
pub fn copy_dir(src: PathBuf, dst: &PathBuf) -> Result<(), io::Error> {
    let mut dirs = vec![src.clone()];
    while let Some(dir) = dirs.pop() {
        let entries = match read_dir(dir) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Fail to readdir: {}", e);
                return Err(e);
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("Invalid dir entry: {}", e);
                    return Err(e);
                }
            };
            let path = entry.path();
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Failed to get file metadata: {}", e);
                    return Err(e);
                }
            };
            if meta.is_dir() {
                dirs.push(path);
                continue;
            }
            let new_path = match path.strip_prefix(&src) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Failed to strip path: {}", e);
                    return Err(io::Error::new(io::ErrorKind::Other, e));
                }
            };
            let new_path = dst.join(new_path);
            if let Some(parent) = new_path.parent() {
                if let Err(e) = create_dir_all(parent) {
                    eprintln!("Failed to create dir: {}", e);
                    return Err(e);
                }
            }
            if meta.is_file() {
                if let Err(e) = copy(path, new_path) {
                    eprintln!("Failed to copy: {}", e);
                    return Err(e);
                }
            } else if meta.is_symlink() {
                // MKDIR PATH
                let link_to = match read_link(&path) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("Failed to read link: {} ", e);
                        return Err(e);
                    }
                };
                if let Err(e) = fs::symlink(link_to, new_path) {
                    eprintln!("Failed to create symlink: {}", e);
                    return Err(e);
                }
            } else {
                // unknow file
            }
        }
    }
    Ok(())
}
