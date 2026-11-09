use std::io;

/// File names are relative to the card root; reads return one file's bytes.
pub trait CardSource {
    fn list(&self) -> io::Result<Vec<String>>;
    fn read(&self, path: &str) -> io::Result<Vec<u8>>;
}

#[cfg(feature = "fs")]
#[derive(Clone, Debug)]
pub struct FsCardSource {
    root: std::path::PathBuf,
}
#[cfg(feature = "fs")]
impl FsCardSource {
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self { root: root.into() }
    }
}
#[cfg(feature = "fs")]
impl CardSource for FsCardSource {
    fn list(&self) -> io::Result<Vec<String>> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let ty = entry.file_type()?;
            if ty.is_dir() {
                for child in std::fs::read_dir(entry.path())? {
                    let child = child?;
                    let child_name = child.file_name().to_string_lossy().into_owned();
                    if !child_name.starts_with('.') && child.file_type()?.is_file() {
                        files.push(format!("{name}/{child_name}"));
                    }
                }
            } else if ty.is_file() {
                files.push(name);
            }
        }
        files.sort();
        Ok(files)
    }
    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        let mut relative = std::path::PathBuf::new();
        for part in path.split(['/', '\\']) {
            if part.is_empty() || part == "." {
                continue;
            }
            if part == ".." || part.contains(':') {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "card path must be relative",
                ));
            }
            relative.push(part);
        }
        if path.starts_with(['/', '\\']) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "card path must be relative",
            ));
        }
        let root = self.root.canonicalize()?;
        let target = root.join(relative).canonicalize()?;
        if !target.starts_with(&root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "card file resolves outside root",
            ));
        }
        std::fs::read(target)
    }
}
