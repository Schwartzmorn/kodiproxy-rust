use sha2::Digest;

pub struct FileRepository {
    root_path: std::path::PathBuf,
}

/// Internal class to handle a single file
pub struct SingleFileRepository {
    root_path: std::path::PathBuf,
    file_dir: std::path::PathBuf,
}

fn digest(data: &Vec<u8>) -> String {
    base64::encode(sha2::Sha256::digest(&data).to_vec())
}

impl FileRepository {
    pub fn new<T>(root_path: T) -> std::sync::Arc<FileRepository>
    where
        T: std::convert::Into<std::path::PathBuf>,
    {
        let root_path = root_path.into();
        if root_path.exists() && !root_path.is_dir() {
            log::error!("Path {:?} exists and is not a directory.", root_path);
            panic!("Path for the file handler is not a directory.")
        }
        if !root_path.exists() {
            std::fs::create_dir_all(&root_path)
                .expect("Could not create root directory for FileHandler");
        }
        std::sync::Arc::new(FileRepository { root_path })
    }

    pub fn get_single_file_repo<T>(
        &self,
        file_dir: T,
        create_if_not_present: bool,
    ) -> Result<SingleFileRepository, router::RouterError>
    where
        std::path::PathBuf: std::convert::From<T>,
    {
        let file_dir = std::path::PathBuf::from(file_dir);
        let full_dir = self.root_path.join(&file_dir);
        if !full_dir.exists() {
            if create_if_not_present {
                log::info!("Creating dir {:?}", full_dir);
                std::fs::create_dir_all(&full_dir)
                    .map_err(|e| super::map_error(&e, "Could not create directory", 500))?
            } else {
                return Err(router::NotFound);
            }
        }
        if !full_dir.is_dir() {
            return Err(router::NotFound);
        }
        Ok(SingleFileRepository {
            root_path: self.root_path.clone(),
            file_dir,
        })
    }
}

impl SingleFileRepository {
    fn get_version_number(entry: &std::fs::DirEntry) -> Option<u32> {
        if entry.path().is_dir() {
            return None;
        }
        match entry.file_name().into_string().map(|s| s.parse::<u32>()) {
            Ok(val) => match val {
                Ok(val) => Some(val),
                _ => None,
            },
            _ => None,
        }
    }

    fn get_path(&self) -> std::path::PathBuf {
        self.root_path.join(&self.file_dir)
    }

    fn get_current_path(&self) -> std::path::PathBuf {
        self.get_path().join("current")
    }

    fn get_log_writer(&self) -> Option<crate::log::FileLogWriter> {
        let log = crate::log::FileLogWriter::new(self.get_path());
        if let Err(e) = &log {
            log::warn!("Could not open log file {:?}", e);
        }
        log.ok()
    }

    fn log(
        &self,
        log: Option<crate::log::FileLogWriter>,
        entry_type: crate::log::FileLogEntryType,
    ) {
        if let Some(mut log) = log {
            let res = log.append(entry_type);
            if let Err(e) = res {
                log::warn!("Failed to write log file '{:?}': {:?}", self.file_dir, e);
            }
        }
    }

    #[allow(dead_code)]
    pub fn get_log(&self) -> Result<crate::log::FileLog, std::io::Error> {
        crate::log::FileLog::new(self.get_path())
    }

    fn delete_no_log(&self) -> Result<(), router::RouterError> {
        let version_number = self.get_current_version_number()?;
        let path_to = self.get_path().join(version_number.to_string());

        std::fs::rename(self.get_current_path(), path_to)
            .map_err(|e| super::map_error(&e, "Error while moving", 500))?;

        Ok(())
    }

    fn save_no_log(&self, data: &Vec<u8>) -> Result<(bool, u32), router::RouterError> {
        let mut version_number = self.get_current_version_number()?;
        let version = version_number.to_string();

        let mut is_create = true;

        if self.get_current_path().exists() {
            is_create = false;
            version_number += 1;
            let path_to = self.get_path().join(version);
            std::fs::rename(self.get_current_path(), path_to)
                .map_err(|e| super::map_error(&e, "Failed to rename current file", 500))?;
        }

        std::fs::write(self.get_current_path(), data)
            .map_err(|e| super::map_error(&e, "Failed to write file", 500))?;

        Ok((is_create, version_number))
    }

    pub fn get_current_version_number(&self) -> Result<u32, router::RouterError> {
        let mut version: Option<u32> = None;
        let path = self.get_path();
        for entry in std::fs::read_dir(self.get_path()).map_err(|e| {
            super::map_error(
                &e,
                format!("Failed to list files in '{:?}'", path).as_str(),
                500,
            )
        })? {
            match entry {
                Ok(entry) => {
                    version = version.max(SingleFileRepository::get_version_number(&entry));
                }
                Err(e) => {
                    log::warn!("Ignored file: {}", e);
                }
            }
        }
        Ok(version.map_or_else(|| 0, |v| v + 1))
    }

    pub fn get_filename(&self) -> Result<String, router::RouterError> {
        match self.get_path().file_name().map(|n| n.to_str()).flatten() {
            Some(name) => Ok(name.to_owned()),
            None => Err(router::HandlerError(
                400,
                format!("Invalid file name '{:?}'", self.file_dir),
            )),
        }
    }

    pub fn get_current_version(&self) -> Result<Vec<u8>, router::RouterError> {
        if self.get_current_path().exists() {
            std::fs::read(self.get_current_path())
                .map_err(|e| super::map_error(&e, "Could not read latest version", 500))
        } else {
            Err(router::NotFound)
        }
    }

    pub fn save(&self, data: &Vec<u8>) -> Result<(), router::RouterError> {
        let log = self.get_log_writer();

        let (is_create, version) = self.save_no_log(data)?;

        let hash = digest(data);

        if is_create {
            self.log(log, crate::log::FileLogEntryType::Creation(version, hash));
        } else {
            self.log(
                log,
                crate::log::FileLogEntryType::Modification(version, hash),
            );
        };

        Ok(())
    }

    pub fn delete(&self) -> Result<(), router::RouterError> {
        let log = self.get_log_writer();

        self.delete_no_log()?;

        self.log(log, crate::log::FileLogEntryType::Deletion());

        Ok(())
    }

    pub fn rename(&self, to_repo: &SingleFileRepository) -> Result<(), router::RouterError> {
        let log_from = self.get_log_writer();
        let log_to = to_repo.get_log_writer();

        let file_content = self.get_current_version()?;

        let (_, version) = to_repo.save_no_log(&file_content)?;

        self.delete_no_log()?;

        self.log(
            log_from,
            crate::log::FileLogEntryType::MoveTo(to_repo.file_dir.to_owned()),
        );

        let hash = digest(&file_content);

        to_repo.log(
            log_to,
            crate::log::FileLogEntryType::MoveFrom(version, hash, self.file_dir.to_owned()),
        );

        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    fn to(content: &str) -> std::vec::Vec<u8> {
        std::vec::Vec::from(content.as_bytes())
    }

    static TEST_PATH: &str = "target/test/repository_tests";

    #[rstest::fixture]
    fn file_repo(#[default("test")] test_name: &str) -> crate::tests::TestRepo {
        crate::tests::TestRepo::new(std::path::PathBuf::from(TEST_PATH).join(test_name))
    }

    #[rstest::rstest]
    fn it_allows_saving(#[with("saving")] file_repo: crate::tests::TestRepo) {
        assert!(!file_repo.get_path("test/new_file.txt").exists());

        let repo = file_repo
            .get_repo()
            .get_single_file_repo("test/new_file.txt", true)
            .unwrap();

        repo.save(to("version 0").as_ref()).unwrap();

        assert!(file_repo.get_path("test/new_file.txt").is_dir());
        assert!(file_repo
            .get_path("test/new_file.txt")
            .join("current")
            .is_file());
        assert_eq!(to("version 0"), repo.get_current_version().unwrap());

        repo.save(to("version 1").as_ref()).unwrap();

        assert!(file_repo.get_path("test/new_file.txt").join("0").is_file());
        assert!(file_repo
            .get_path("test/new_file.txt")
            .join("current")
            .is_file());
        assert_eq!(to("version 1"), repo.get_current_version().unwrap());

        let log = repo.get_log().unwrap();

        assert_eq!(2, log.entries.len());
        assert!(std::matches!(
            &log.entries[0].entry_type,
            crate::log::FileLogEntryType::Creation(0, _)
        ));
        assert!(std::matches!(
            &log.entries[1].entry_type,
            crate::log::FileLogEntryType::Modification(1, _)
        ));
    }

    #[rstest::rstest]
    fn it_allows_deleting(#[with("deleting")] file_repo: crate::tests::TestRepo) {
        let repo = file_repo
            .get_repo()
            .get_single_file_repo("new_file.txt", true)
            .unwrap();

        repo.save(to("version 0").as_ref()).unwrap();

        repo.delete().unwrap();

        assert!(file_repo.get_path("new_file.txt").join("0").is_file());
        assert!(!file_repo.get_path("new_file.txt").join("current").exists());

        assert_eq!(Err(router::NotFound), repo.get_current_version());

        let log = repo.get_log().unwrap();

        assert_eq!(2, log.entries.len());
        assert!(std::matches!(
            &log.entries[0].entry_type,
            crate::log::FileLogEntryType::Creation(0, _)
        ));
        assert!(std::matches!(
            &log.entries[1].entry_type,
            crate::log::FileLogEntryType::Deletion()
        ));
    }

    #[rstest::rstest]
    fn it_allows_moving(#[with("moving")] file_repo: crate::tests::TestRepo) {
        let repo = file_repo
            .get_repo()
            .get_single_file_repo("new_file.txt.tmp", true)
            .unwrap();

        repo.save(to("version 0").as_ref()).unwrap();

        let to_repo = file_repo
            .get_repo()
            .get_single_file_repo("new_file.txt", true)
            .unwrap();

        repo.rename(&to_repo).unwrap();

        assert_eq!(Err(router::NotFound), repo.get_current_version());
        assert_eq!(to("version 0"), to_repo.get_current_version().unwrap());

        let log_from = repo.get_log().unwrap();
        let log_to = to_repo.get_log().unwrap();

        assert_eq!(2, log_from.entries.len());
        assert!(std::matches!(
            &log_from.entries[1].entry_type,
            crate::log::FileLogEntryType::MoveTo(path) if path == &std::path::PathBuf::from("new_file.txt")
        ));

        assert_eq!(1, log_to.entries.len());
        assert!(std::matches!(
            &log_to.entries[0].entry_type,
            crate::log::FileLogEntryType::MoveFrom(_, _, path) if path == &std::path::PathBuf::from("new_file.txt.tmp")
        ));
    }
}
