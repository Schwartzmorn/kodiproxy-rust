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
