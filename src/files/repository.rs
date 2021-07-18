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

    fn get_full_dir<T>(&self, file_dir: T) -> std::path::PathBuf
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
                return Err(crate::router::NotFound);
            }
        }
        if !file_dir.is_dir() {
            return Err(crate::router::NotFound);
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

    fn get_current_path(&self) -> std::path::PathBuf {
        self.file_dir.join("current")
    }

    pub fn get_current_version_number(&self) -> Result<i32, crate::router::RouterError> {
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
        Ok(version.map_or_else(|| 0, |v| v + 1))
    }

    pub fn get_filename(&self) -> Result<&str, crate::router::RouterError> {
        match self.file_dir.file_name().map(|n| n.to_str()).flatten() {
            Some(name) => Ok(name),
            None => Err(crate::router::HandlerError(
                400,
                format!("Invalid file name '{:?}'", self.file_dir),
            )),
        }
    }

    pub fn get_current_version(&self) -> Result<Vec<u8>, crate::router::RouterError> {
        if self.get_current_path().exists() {
            std::fs::read(self.get_current_path())
                .map_err(|e| super::map_error(&e, "Could not read latest version", 500))
        } else {
            Err(crate::router::NotFound)
        }
    }

    pub fn save(&self, data: &Vec<u8>) -> Result<(), crate::router::RouterError> {
        let version = self.get_current_version_number()?.to_string();

        let path_to = self.file_dir.join(version);

        if self.get_current_path().exists() {
            std::fs::rename(self.get_current_path(), path_to)
                .map_err(|e| super::map_error(&e, "Failed to rename current file", 500))?;
        }

        std::fs::write(self.get_current_path(), data)
            .map_err(|e| super::map_error(&e, "Failed to write file", 500))?;

        Ok(())
    }

    pub fn delete(&self) -> Result<(), crate::router::RouterError> {
        let version_number = self.get_current_version_number()?;
        let path_to = self.file_dir.join(version_number.to_string());

        std::fs::rename(self.get_current_path(), path_to)
            .map_err(|e| super::map_error(&e, "Error while moving", 500))?;

        Ok(())
    }

    pub fn rename(&self, to_repo: &SingleFileRepository) -> Result<(), crate::router::RouterError> {
        let file_content = self.get_current_version()?;
        to_repo.save(&file_content)?;
        self.delete()?;
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {

    fn to(content: &str) -> std::vec::Vec<u8> {
        std::vec::Vec::from(content.as_bytes())
    }

    pub struct TestRepo {
        test_path: std::path::PathBuf,
        repo: std::sync::Arc<super::FileRepository>,
    }

    impl TestRepo {
        pub fn new(test_path: std::path::PathBuf) -> TestRepo {
            TestRepo::clean(&test_path);
            TestRepo {
                test_path: test_path.clone(),
                repo: super::FileRepository::new(test_path),
            }
        }

        pub fn get_path(&self, file_name: &str) -> std::path::PathBuf {
            self.test_path.join(file_name)
        }

        pub fn get_repo(&self) -> std::sync::Arc<super::FileRepository> {
            self.repo.clone()
        }

        fn clean(path: &std::path::PathBuf) {
            if path.exists() {
                std::fs::remove_dir_all(path).unwrap();
            }
        }
    }

    impl Drop for TestRepo {
        fn drop(&mut self) {
            TestRepo::clean(&self.test_path);
        }
    }

    static TEST_PATH: &str = "target/test/repository_tests";

    #[rstest::fixture]
    fn file_repo(#[default("test")] test_name: &str) -> crate::files::TestRepo {
        crate::files::TestRepo::new(std::path::PathBuf::from(TEST_PATH).join(test_name))
    }

    #[rstest::rstest]
    fn it_allows_saving(#[with("saving")] file_repo: crate::files::TestRepo) {
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
    }

    #[rstest::rstest]
    fn it_allows_deleting(#[with("deleting")] file_repo: crate::files::TestRepo) {
        let repo = file_repo
            .get_repo()
            .get_single_file_repo("new_file.txt", true)
            .unwrap();

        repo.save(to("version 0").as_ref()).unwrap();

        repo.delete().unwrap();

        assert!(file_repo.get_path("new_file.txt").join("0").is_file());
        assert!(!file_repo.get_path("new_file.txt").join("current").exists());

        assert_eq!(Err(crate::router::NotFound), repo.get_current_version());
    }

    #[rstest::rstest]
    fn it_allows_moving(#[with("moving")] file_repo: crate::files::TestRepo) {
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

        assert_eq!(Err(crate::router::NotFound), repo.get_current_version());
        assert_eq!(to("version 0"), to_repo.get_current_version().unwrap());
    }
}
