use sha2::Digest;

// Setup statements
static SQL_CREATE_FILES_TABLE: &str = "create table if not exists FILES (
    PATH text not null,
    NAME text not null,
    VERSION integer not null,
    TIMESTAMP integer not null,
    HASH text not null,
    FILE blob not null,
    primary key (PATH, NAME)
)";

static SQL_CREATE_FILES_HISTORY_TABLE: &str = "create table if not exists FILES_HISTORY (
    PATH text not null,
    NAME text not null,
    VERSION integer not null,
    TIMESTAMP integer not null,
    OPERATION text not null,
    IP_ADDRESS text not null,
    HASH text,
    OLD_OR_NEW_PATH text,
    FILE blob,
    primary key (PATH, NAME, VERSION)
)";

// FILES statements
static SQL_UPSERT_FILE: &str = "insert into FILES (PATH, NAME, VERSION, TIMESTAMP, HASH, FILE)
    values (?, ?, ?, ?, ?, ?)
    on conflict(PATH, NAME) do update
    set VERSION=excluded.VERSION, TIMESTAMP=excluded.TIMESTAMP, HASH=excluded.HASH, FILE=excluded.FILE";

static SQL_DELETE_FILE: &str = "delete from FILES where PATH=? and NAME=?";

static SQL_SELECT_FILE: &str = "select VERSION, TIMESTAMP, FILE from FILES where PATH=? and NAME=?";

static SQL_SELECT_VERSION: &str = "select VERSION from FILES where PATH=? and NAME=?";

static SQL_SELECT_FILE_NO_CONTENT: &str =
    "select VERSION, TIMESTAMP from FILES where PATH=? and NAME=?";

// FILES_HISTORY statements
static SQL_INSERT_HISTORY_LINE: &str = "insert into FILES_HISTORY
    (PATH, NAME, VERSION, TIMESTAMP, OPERATION, IP_ADDRESS, HASH, OLD_OR_NEW_PATH, FILE)
    values (?, ?, ?, ?, ?, ?, ?, ?, ?)";

static SQL_SELECT_HISTORY_VERSION: &str =
    "select max(VERSION) from FILES_HISTORY where PATH=? and NAME=?";

static SQL_SELECT_HISTORY: &str =
    "select VERSION, TIMESTAMP, OPERATION, IP_ADDRESS, HASH, OLD_OR_NEW_PATH from FILES_HISTORY
    where PATH=? and NAME=? order by VERSION";

/// Contains the current state of a resource
#[derive(Debug)]
pub struct FilesDbResponse {
    /// Current version number of the resource
    pub version: i32,
    /// Timestamp of the last modification
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Only present in the response of [`FilesDB::get()`], contains the resource
    pub file: Option<Vec<u8>>,
}

pub struct FilesDB {
    connection: rusqlite::Connection,
}

impl FilesDB {
    pub fn new<T>(root_path: T) -> Result<FilesDB, router::RouterError>
    where
        T: std::convert::Into<std::path::PathBuf>,
    {
        let root_path: std::path::PathBuf = root_path.into();
        if root_path.exists() && !root_path.is_dir() {
            log::error!("Path {:?} exists and is not a directory.", root_path);
            return Err(router::RouterError::HandlerError(
                500,
                String::from("File repository path exists and is not a directory"),
            ));
        }
        if !root_path.exists() {
            log::info!("Creating path: {:?}", root_path);
            std::fs::create_dir_all(&root_path).map_err(|e| {
                router::RouterError::HandlerError(
                    500,
                    format!("Could not create directory for file repository: {}", e),
                )
            })?;
        }
        let db_path = root_path.join("file_repository.db3");
        if db_path.exists() {
            log::info!("Database already exists")
        }

        log::info!("Opening database in {:?}", db_path);
        let connection = rusqlite::Connection::open(db_path);
        let connection = map_sqlite_result(connection, "Failed to open sqlite database")?;

        let result = connection.execute(SQL_CREATE_FILES_TABLE, []);
        map_sqlite_result(result, "Failed to create FILES table in sqlite database")?;

        let result = connection.execute(SQL_CREATE_FILES_HISTORY_TABLE, []);
        map_sqlite_result(
            result,
            "Failed to create FILES_HISTORY table in sqlite database",
        )?;

        Ok(FilesDB { connection })
    }

    /// Retrieves the latest version of a resource
    /// if get_content is false, only the version and timestamp will be retrieved
    pub fn get(
        &self,
        file_path: &str,
        file_name: &str,
        get_content: bool,
    ) -> Result<FilesDbResponse, router::RouterError> {
        self.connection
            .query_row(
                if get_content {
                    SQL_SELECT_FILE
                } else {
                    SQL_SELECT_FILE_NO_CONTENT
                },
                rusqlite::params![file_path, file_name],
                |row| {
                    Ok(FilesDbResponse {
                        version: row.get(0)?,
                        timestamp: decode_timestamp(row.get(1)?)?,
                        file: if get_content { Some(row.get(2)?) } else { None },
                    })
                },
            )
            .map_err(|error| super::map_error(&error, "Could not find file", 404))
    }

    /// Moves a resource
    /// If successful, the [FilesDbResponse] will contain the state of the initial resource
    pub fn move_to(
        &mut self,
        file_path_from: &str,
        file_name_from: &str,
        file_version_from: i32,
        file_path_to: &str,
        file_name_to: &str,
        address: &std::net::IpAddr,
    ) -> Result<FilesDbResponse, router::RouterError> {
        if file_name_from == file_name_to && file_path_from == file_path_to {
            return Err(router::InvalidRequest(String::from(
                "Origin and destination are the same",
            )));
        }

        let db_version_from = self
            .get_current_version(file_path_from, file_name_from)
            .ok_or(router::HandlerError(404, String::from("File not found")))?;
        if db_version_from != file_version_from {
            return Err(router::HandlerError(412, String::from("Version mismatch")));
        }

        let db_version_to = self.get_current_version(file_path_to, file_name_to);
        if let Some(_) = db_version_to {
            return Err(router::HandlerError(
                412,
                String::from("Destination already exists"),
            ));
        }

        let file_data = self
            .get(file_path_from, file_name_from, true)?
            .file
            .ok_or(router::HandlerError(404, String::from("File not found")))?;

        let hash = digest(&file_data);
        let timestamp = chrono::Utc::now();
        let timestamp_str = chrono::Utc::now().to_rfc3339();
        let new_version_from = db_version_from + 1;
        let new_version_to = self
            .get_history_version(file_path_to, file_name_to)
            .map_or(0, |v| v + 1);
        let address = address.to_string();

        let path_from = std::path::PathBuf::from(file_path_from).join(file_name_from);
        let path_to = std::path::PathBuf::from(file_path_to).join(file_name_to);

        log::info!(
            "Starting move transaction from file {}/{} to {}/{}",
            file_path_from,
            file_name_from,
            file_path_to,
            file_name_to,
        );

        let transaction = self
            .connection
            .transaction()
            .map_err(|error| super::map_error(&error, "Failed to move file", 500))?;

        log::debug!("Inserting MOVE_TO history line");
        transaction
            .execute(
                SQL_INSERT_HISTORY_LINE,
                rusqlite::params![
                    file_path_from,
                    file_name_from,
                    new_version_from,
                    timestamp_str,
                    "MOVE_TO",
                    &address,
                    &rusqlite::types::Null,
                    path_to.to_string_lossy(),
                    &rusqlite::types::Null
                ],
            )
            .map_err(|error| super::map_error(&error, "Failed to move file", 500))?;

        log::debug!("Inserting MOVE_FROM history line");
        transaction
            .execute(
                SQL_INSERT_HISTORY_LINE,
                rusqlite::params![
                    file_path_to,
                    file_name_to,
                    new_version_to,
                    timestamp_str,
                    "MOVE_FROM",
                    &address,
                    &hash,
                    path_from.to_string_lossy(),
                    file_data
                ],
            )
            .map_err(|error| super::map_error(&error, "Failed to move file", 500))?;

        log::debug!("Deleting file from old path");
        transaction
            .execute(
                SQL_DELETE_FILE,
                rusqlite::params![file_path_from, file_name_from,],
            )
            .map_err(|error| super::map_error(&error, "Failed to move file", 500))?;

        log::debug!("Creating file in new old path");
        transaction
            .execute(
                SQL_UPSERT_FILE,
                rusqlite::params![
                    file_path_to,
                    file_name_to,
                    new_version_to,
                    timestamp_str,
                    &hash,
                    &file_data
                ],
            )
            .map_err(|error| super::map_error(&error, "Failed to move file", 500))?;

        transaction
            .commit()
            .map_err(|error| super::map_error(&error, "Failed to move file", 500))?;

        Ok(FilesDbResponse {
            version: new_version_to,
            timestamp,
            file: None,
        })
    }

    /// Saves the new version of a resource
    /// This works to update or create a new resource
    pub fn save(
        &mut self,
        file_path: &str,
        file_name: &str,
        file_data: &Vec<u8>,
        file_version: Option<i32>,
        address: &std::net::IpAddr,
    ) -> Result<FilesDbResponse, router::RouterError> {
        let hash = digest(file_data);
        let timestamp = chrono::Utc::now();
        let timestamp_str = timestamp.to_rfc3339();
        let address = address.to_string();

        log::info!(
            "Starting creation transaction for file {}/{} with hash {} and version {:?}",
            file_path,
            file_name,
            &hash,
            file_version
        );

        let db_version = self.get_current_version(file_path, file_name);

        let transaction = self
            .connection
            .transaction()
            .map_err(|error| super::map_error(&error, "Failed to save file", 500))?;

        log::debug!("File version: {:?}", db_version);

        if file_version != db_version {
            return Err(router::RouterError::HandlerError(
                412,
                String::from("Version mismatch"),
            ));
        }

        let new_version = db_version.map(|v| v + 1).unwrap_or(
            transaction
                .query_row(
                    SQL_SELECT_HISTORY_VERSION,
                    rusqlite::params![file_path, file_name],
                    |row| row.get(0),
                )
                .ok()
                .map_or(0, |v: i32| v + 1),
        );

        log::debug!("Inserting history line");
        transaction
            .execute(
                SQL_INSERT_HISTORY_LINE,
                rusqlite::params![
                    file_path,
                    file_name,
                    new_version,
                    timestamp_str,
                    if db_version.is_some() {
                        "UPDATE"
                    } else {
                        "CREATION"
                    },
                    &address,
                    &hash,
                    &rusqlite::types::Null,
                    file_data
                ],
            )
            .map_err(|error| super::map_error(&error, "Failed to save file", 500))?;

        log::debug!("Updating file");
        transaction
            .execute(
                SQL_UPSERT_FILE,
                rusqlite::params![
                    file_path,
                    file_name,
                    new_version,
                    timestamp_str,
                    &hash,
                    file_data
                ],
            )
            .map_err(|error| super::map_error(&error, "Failed to save file", 500))?;

        transaction
            .commit()
            .map_err(|error| super::map_error(&error, "Failed to save file", 500))?;
        Ok(FilesDbResponse {
            version: new_version,
            timestamp,
            file: None,
        })
    }

    /// Deletes a resource
    pub fn delete(
        &mut self,
        file_path: &str,
        file_name: &str,
        file_version: i32,
        address: &std::net::IpAddr,
    ) -> Result<FilesDbResponse, router::RouterError> {
        let timestamp = chrono::Utc::now();
        let timestamp_str = timestamp.to_rfc3339();
        let address = address.to_string();

        log::info!(
            "Starting deletion transaction for file {}/{} with version {}",
            file_path,
            file_name,
            file_version
        );

        let db_version = self
            .get_current_version(file_path, file_name)
            .ok_or(router::HandlerError(404, String::from("File not found")))?;

        let transaction = self
            .connection
            .transaction()
            .map_err(|error| super::map_error(&error, "Failed to delete", 500))?;

        if file_version != db_version {
            return Err(router::RouterError::HandlerError(
                412,
                String::from("Version mismatch"),
            ));
        }

        let new_version = db_version + 1;

        log::debug!("Deleting file");
        let rows_updated = transaction
            .execute(SQL_DELETE_FILE, rusqlite::params![file_path, file_name,])
            .map_err(|error| super::map_error(&error, "Failed to delete file", 500))?;

        if rows_updated != 0 {
            log::debug!("Inserting history line");
            transaction
                .execute(
                    SQL_INSERT_HISTORY_LINE,
                    rusqlite::params![
                        file_path,
                        file_name,
                        new_version,
                        timestamp_str,
                        "DELETION",
                        &address,
                        &rusqlite::types::Null,
                        &rusqlite::types::Null,
                        &rusqlite::types::Null
                    ],
                )
                .map_err(|error| super::map_error(&error, "Failed to delete file", 500))?;
        } else {
            log::debug!("No row deleted");
        }

        transaction
            .commit()
            .map_err(|error| super::map_error(&error, "Failed to delete file", 500))?;

        Ok(FilesDbResponse {
            version: new_version,
            timestamp,
            file: None,
        })
    }

    /// Returns the history of a resource as a [crate::log::FileLog]
    pub fn get_history(
        &self,
        file_path: &str,
        file_name: &str,
    ) -> Result<crate::log::FileLog, router::RouterError> {
        let history = self
            .get_history_inner(file_path, file_name)
            .map_err(|error| super::map_error(&error, "Failed to retrieve history", 500));
        if let Ok(log) = &history {
            if log.entries.is_empty() {
                return Err(router::RouterError::NotFound);
            }
        }
        history
    }

    fn get_current_version(&self, file_path: &str, file_name: &str) -> Option<i32> {
        self.connection
            .query_row(
                SQL_SELECT_VERSION,
                rusqlite::params![file_path, file_name],
                |row| row.get(0),
            )
            .ok()
    }

    fn get_history_version(&self, file_path: &str, file_name: &str) -> Option<i32> {
        self.connection
            .query_row(
                SQL_SELECT_HISTORY_VERSION,
                rusqlite::params![file_path, file_name],
                |row| row.get(0),
            )
            .ok()
    }

    fn get_history_inner(
        &self,
        file_path: &str,
        file_name: &str,
    ) -> Result<crate::log::FileLog, rusqlite::Error> {
        log::info!("Retrieving history for file {}/{}", file_path, file_name);
        let mut statement = self.connection.prepare(SQL_SELECT_HISTORY)?;
        let mut rows = statement.query(rusqlite::params![file_path, file_name])?;
        let mut entries: Vec<crate::log::FileLogEntry> = vec![];
        while let Some(row) = rows.next()? {
            log::debug!(
                "Decoding new history line for file {}/{}",
                file_path,
                file_name
            );
            if let Some(entry) = FilesDB::decode_history_row(row) {
                entries.push(entry);
            } else {
                log::warn!(
                    "Ignoring invalid history line for file {}/{}",
                    file_path,
                    file_name
                );
            }
        }
        Ok(crate::log::FileLog { entries })
    }

    fn decode_history_row(row: &rusqlite::Row) -> Option<crate::log::FileLogEntry> {
        // VERSION, TIMESTAMP, OPERATION, IP_ADDRESS, HASH, OLD_OR_NEW_PATH
        let version: u32 = row.get(0).ok()?;
        log::trace!("Decoding version {}", version);
        let timestamp: String = row.get(1).ok()?;
        log::trace!("Decoding timestamp");
        let timestamp: chrono::DateTime<chrono::Utc> =
            chrono::DateTime::parse_from_rfc3339(timestamp.as_ref())
                .map(|ts| ts.with_timezone(&chrono::Utc))
                .ok()?;
        let operation: String = row.get(2).ok()?;
        let address: String = row.get(3).ok()?;
        log::trace!("Decoding address");
        let address: std::net::IpAddr =
            <std::net::IpAddr as std::str::FromStr>::from_str(address.as_ref()).ok()?;
        let hash: Option<String> = row.get(4).ok()?;
        let path: Option<String> = row.get(5).ok()?;
        log::trace!("Creating entry");
        match crate::log::FileLogEntryType::new(operation, version, hash, path) {
            Ok(entry) => Some(crate::log::FileLogEntry {
                timestamp,
                address,
                entry,
            }),
            Err(msg) => {
                log::info!("Hisory entry was invalid: {}", msg);
                None
            }
        }
    }
}

fn decode_timestamp(timestamp: String) -> Result<chrono::DateTime<chrono::Utc>, rusqlite::Error> {
    chrono::DateTime::parse_from_rfc3339(timestamp.as_ref())
        .map(|ts| ts.with_timezone(&chrono::Utc))
        .map_err(|_| rusqlite::Error::InvalidColumnName(String::from("Failed to decode timestamp")))
}

fn map_sqlite_result<T, E>(result: Result<T, E>, message: &str) -> Result<T, router::RouterError>
where
    E: std::fmt::Debug,
{
    if let Err(e) = &result {
        log::info!("{}: {:?}", message, e);
    }
    result.map_err(|e| router::RouterError::HandlerError(500, format!("{}: {:?}", message, e)))
}

fn digest(data: &Vec<u8>) -> String {
    base64::encode(sha2::Sha256::digest(&data).to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;
    static TEST_PATH: &str = "target/test/files/db";

    fn get_repo(path: &str) -> FilesDB {
        let path = std::path::PathBuf::from(TEST_PATH).join(path);
        if path.exists() {
            std::fs::remove_dir_all(&path)
                .expect(format!("Failed to clean folder {:?}", path).as_str());
        }
        FilesDB::new(path).unwrap()
    }

    #[test]
    fn it_allows_opening_and_reopening() {
        let mut db = get_repo("opening");
        let address = std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));
        let file_path = "test/path";
        let file_name = "test_filename";
        let file_data = std::vec::Vec::from("SOME_DATA".as_bytes());

        db.save(file_path, file_name, &file_data, None, &address)
            .unwrap();

        // We reopen the the same database and check we indeed have our file inside
        let db = FilesDB::new(std::path::PathBuf::from(TEST_PATH).join("opening")).unwrap();
        let retrieved_data = db.get(file_path, file_name, true).unwrap();

        assert_eq!(0, retrieved_data.version);
        assert_eq!(file_data, retrieved_data.file.unwrap());
    }

    #[test]
    fn it_allows_saving_and_resaving() {
        let mut db = get_repo("saving");
        let first_address = std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));
        let file_path = "test/path";
        let file_name = "test_filename";
        let file_data_1 = std::vec::Vec::from("SOME_DATA_1".as_bytes());
        let file_data_2 = std::vec::Vec::from("SOME_DATA_2".as_bytes());

        db.save(file_path, file_name, &file_data_1, None, &first_address)
            .unwrap();

        let saved_data = db.get(file_path, file_name, true).unwrap();

        assert_eq!(0, saved_data.version);
        assert_eq!(file_data_1, saved_data.file.unwrap());

        db.save(file_path, file_name, &file_data_2, Some(0), &first_address)
            .unwrap();

        let saved_data = db.get(file_path, file_name, true).unwrap();

        assert_eq!(1, saved_data.version);
        assert_eq!(file_data_2, saved_data.file.unwrap());
    }

    #[test]
    fn it_allows_deleting() {
        let mut db = get_repo("deleting");
        let address = std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));
        let file_path = "test/path";
        let file_name = "test_filename";
        let file_data_1 = std::vec::Vec::from("SOME_DATA".as_bytes());

        db.save(file_path, file_name, &file_data_1, None, &address)
            .unwrap();

        db.delete(file_path, file_name, 0, &address).unwrap();

        let error = db.get(file_path, file_name, true).unwrap_err();

        assert!(matches!(error, router::RouterError::HandlerError(404, _)));
    }

    #[test]
    fn it_allows_moving() {
        let mut db = get_repo("moving");
        let address = std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));
        let file_path = "test/path";
        let file_name = "test_filename";
        let file_path_to = "test/path_to";
        let file_name_to = "test_filename_to";
        let file_data = std::vec::Vec::from("SOME_DATA".as_bytes());

        db.save(file_path, file_name, &file_data, None, &address)
            .unwrap();

        db.move_to(
            file_path,
            file_name,
            0,
            file_path_to,
            file_name_to,
            &address,
        )
        .unwrap();

        let error = db.get(file_path, file_name, true).unwrap_err();

        assert!(matches!(error, router::RouterError::HandlerError(404, _)));

        let saved_data = db.get(file_path_to, file_name_to, true).unwrap();

        assert_eq!(file_data, saved_data.file.unwrap());
    }

    #[test]
    fn it_tracks_history() {
        let mut db = get_repo("history");
        let address = std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));
        let file_path = "test/path";
        let file_name = "test_filename";
        let file_path_to = "test/path_to";
        let file_name_to = "test_filename_to";
        let file_data_1 = std::vec::Vec::from("SOME_DATA_1".as_bytes());
        let file_data_2 = std::vec::Vec::from("SOME_DATA_1".as_bytes());
        let file_data_3 = std::vec::Vec::from("SOME_DATA_3".as_bytes());
        let file_data_4 = std::vec::Vec::from("SOME_DATA_4".as_bytes());

        db.save(file_path, file_name, &file_data_1, None, &address)
            .unwrap();
        db.save(file_path, file_name, &file_data_2, Some(0), &address)
            .unwrap();
        db.delete(file_path, file_name, 1, &address).unwrap();
        db.delete(file_path, file_name, 1, &address).unwrap_err(); // we delete twice to check the second time does not add anything in history
        db.save(file_path, file_name, &file_data_3, None, &address)
            .unwrap();
        db.move_to(
            file_path,
            file_name,
            3,
            file_path_to,
            file_name_to,
            &address,
        )
        .unwrap();
        db.save(file_path, file_name, &file_data_4, None, &address)
            .unwrap();
        db.save(file_path_to, file_name_to, &file_data_4, Some(0), &address)
            .unwrap();

        let history_from = db.get_history(file_path, file_name).unwrap();

        let history_to = db.get_history(file_path_to, file_name_to).unwrap();

        assert_eq!(history_from.entries.len(), 6);
        assert_eq!(history_to.entries.len(), 2);

        assert_matches::assert_matches!(
            history_from.entries[..],
            [
                crate::log::FileLogEntry {
                    entry: crate::log::FileLogEntryType::Creation { version: 0, .. },
                    ..
                },
                crate::log::FileLogEntry {
                    entry: crate::log::FileLogEntryType::Update { version: 1, .. },
                    ..
                },
                crate::log::FileLogEntry {
                    entry: crate::log::FileLogEntryType::Deletion { version: 2 },
                    ..
                },
                crate::log::FileLogEntry {
                    entry: crate::log::FileLogEntryType::Creation { version: 3, .. },
                    ..
                },
                crate::log::FileLogEntry {
                    entry: crate::log::FileLogEntryType::MoveTo { version: 4, .. },
                    ..
                },
                crate::log::FileLogEntry {
                    entry: crate::log::FileLogEntryType::Creation { version: 5, .. },
                    ..
                }
            ]
        );

        assert_matches::assert_matches!(
            history_to.entries[..],
            [
                crate::log::FileLogEntry {
                    entry: crate::log::FileLogEntryType::MoveFrom { version: 0, .. },
                    ..
                },
                crate::log::FileLogEntry {
                    entry: crate::log::FileLogEntryType::Update { version: 1, .. },
                    ..
                }
            ]
        );
    }

    #[test]
    fn it_prevents_deleting_when_version_is_wrong() {
        let mut db = get_repo("version_delete");
        let address = std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));
        let file_path = "test/path";
        let file_name = "test_filename";
        let file_data_1 = std::vec::Vec::from("SOME_DATA_1".as_bytes());
        let file_data_2 = std::vec::Vec::from("SOME_DATA_2".as_bytes());

        db.save(file_path, file_name, &file_data_1, None, &address)
            .unwrap();
        db.save(file_path, file_name, &file_data_2, Some(0), &address)
            .unwrap();
        let error = db.delete(file_path, file_name, 0, &address).unwrap_err();

        assert!(matches!(error, router::RouterError::HandlerError(412, _)));
    }

    #[test]
    fn it_prevents_saving_when_version_is_wrong() {
        let mut db = get_repo("version_save");
        let address = std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));
        let file_path = "test/path";
        let file_name = "test_filename";
        let file_data_1 = std::vec::Vec::from("SOME_DATA_1".as_bytes());
        let file_data_2 = std::vec::Vec::from("SOME_DATA_2".as_bytes());

        db.save(file_path, file_name, &file_data_1, None, &address)
            .unwrap();
        let error = db
            .save(file_path, file_name, &file_data_2, None, &address)
            .unwrap_err();

        assert!(matches!(error, router::RouterError::HandlerError(412, _)));

        db.save(file_path, file_name, &file_data_1, Some(0), &address)
            .unwrap();
        let error = db
            .save(file_path, file_name, &file_data_1, None, &address)
            .unwrap_err();

        assert!(matches!(error, router::RouterError::HandlerError(412, _)));
    }

    #[test]
    fn it_prevents_moving_when_version_is_wrong() {
        let mut db = get_repo("move_wrong_version");
        let address = std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));
        let file_path = "test/path";
        let file_name = "test_filename";
        let file_data = std::vec::Vec::from("SOME_DATA".as_bytes());

        db.save(file_path, file_name, &file_data, None, &address)
            .unwrap();

        let error = db
            .move_to(file_path, file_name, 1, file_path, "test_to", &address)
            .unwrap_err();

        assert!(matches!(error, router::RouterError::HandlerError(412, _)));
    }

    #[test]
    fn it_prevents_moving_when_destination_exists() {
        let mut db = get_repo("move_destination_exists");
        let address = std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));
        let file_path = "test/path";
        let file_name = "test_filename";
        let file_name_to = "test_filename_to";
        let file_data = std::vec::Vec::from("SOME_DATA".as_bytes());

        db.save(file_path, file_name, &file_data, None, &address)
            .unwrap();
        db.save(file_path, file_name_to, &file_data, None, &address)
            .unwrap();

        let error = db
            .move_to(file_path, file_name, 0, file_path, file_name_to, &address)
            .unwrap_err();

        assert!(matches!(error, router::RouterError::HandlerError(412, _)));
    }
}
