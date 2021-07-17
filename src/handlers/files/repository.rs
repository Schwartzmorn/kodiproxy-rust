pub struct FileRepository {
    root_path: std::path::PathBuf,
}

/// Internal class to handle a single file
pub struct SingleFileRepository {
    file_dir: std::path::PathBuf,
}

impl FileRepository {
    pub fn new<T>(root_path: T) -> std::sync::Arc<FileRepository>
    where
        T: std::convert::Into<std::path::PathBuf>,
    {
        let root_path: std::path::PathBuf = root_path.into();
        if root_path.exists() && !root_path.is_dir() {
            eprintln!("Path {:?} is not a directory.", root_path);
            panic!("Path for the file handler is not a directory.")
        }
        if !root_path.exists() {
            std::fs::create_dir_all(&root_path)
                .expect("Could not create root directory for FileHandler");
        }
        std::sync::Arc::new(FileRepository { root_path })
    }

    // TODO make this private
    pub fn get_full_dir<T>(&self, file_dir: T) -> std::path::PathBuf
    where
        std::path::PathBuf: std::convert::From<T>,
    {
        self.root_path.join(std::path::PathBuf::from(file_dir))
    }

    pub fn get_single_file_repo<T>(
        &self,
        file_dir: T,
        create_if_not_present: bool,
    ) -> Result<SingleFileRepository, crate::router::RouterError>
    where
        std::path::PathBuf: std::convert::From<T>,
    {
        let file_dir = self.get_full_dir(file_dir);
        if !file_dir.exists() {
            if create_if_not_present {
                std::fs::create_dir_all(&file_dir)
                    .map_err(|e| super::map_error(&e, "Could not create directory", 500))?
            } else {
                return Err(crate::router::RouterError::NotFound);
            }
        }
        if !file_dir.is_dir() {
            return Err(crate::router::RouterError::NotFound);
        }
        Ok(SingleFileRepository { file_dir })
    }
}

impl SingleFileRepository {
    fn get_version_number(entry: &std::fs::DirEntry) -> Option<i32> {
        if entry.path().is_dir() {
            return None;
        }
        match entry.file_name().into_string().map(|s| s.parse::<i32>()) {
            Ok(val) => match val {
                Ok(val) => Some(val),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn get_current_version_number(&self) -> Result<Option<i32>, crate::router::RouterError> {
        let mut version: Option<i32> = None;
        for entry in std::fs::read_dir(&self.file_dir)
            .map_err(|e| super::map_error(&e, "Failed to list files", 500))?
        {
            match entry {
                Ok(entry) => {
                    version = version.max(SingleFileRepository::get_version_number(&entry));
                }
                Err(e) => {
                    log::warn!("Ignored file: {}", e);
                }
            }
        }
        Ok(version)
    }

    pub fn get_filename(&self) -> Result<&str, crate::router::RouterError> {
        match self.file_dir.file_name().map(|n| n.to_str()).flatten() {
            Some(name) => Ok(name),
            None => Err(crate::router::RouterError::HandlerError(
                400,
                format!("Invalid file name '{:?}'", self.file_dir),
            )),
        }
    }

    pub fn get_current_version(&self) -> Result<Vec<u8>, crate::router::RouterError> {
        if let Some(version) = self.get_current_version_number()? {
            let full_path = self.file_dir.join(version.to_string());
            std::fs::read(full_path)
                .map_err(|e| super::map_error(&e, "Could not read latest version", 500))
        } else {
            Err(crate::router::RouterError::NotFound)
        }
    }

    pub fn save(&self, data: &Vec<u8>) -> Result<(), crate::router::RouterError> {
        let version = self
            .get_current_version_number()?
            .map_or(0, |v| v + 1)
            .to_string();

        let path = self.file_dir.join(version);

        std::fs::write(path, data)
            .map_err(|e| super::map_error(&e, "Failed to write file", 500))?;

        Ok(())
    }

    pub fn delete(&self) -> Result<(), crate::router::RouterError> {
        std::fs::remove_dir_all(&self.file_dir)
            .map_err(|e| super::map_error(&e, "Failed to delete file", 500))?;
        Ok(())
    }

    pub fn rename(
        self,
        new_path: &std::path::PathBuf,
    ) -> Result<SingleFileRepository, crate::router::RouterError> {
        std::fs::rename(self.file_dir, new_path)
            .map_err(|e| super::map_error(&e, "Error while moving", 500))?;
        Ok(SingleFileRepository {
            file_dir: new_path.to_owned(),
        })
    }
}
