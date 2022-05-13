use sha2::Digest;

static SQL_CREATE_FILES_TABLE: &str = "create table if not exists FILES (
    PATH text not null,
    NAME text not null,
    HASH text,
    IS_SYNCED integer not null,
    LAST_SYNCED_VERSION integer,
    LAST_SYNCED_TIMESTAMP integer,
    FILE blob,
    primary key (PATH, NAME)
)";

static SQL_SELECT_FILE: &str = "select 
    HASH, IS_SYNCED, LAST_SYNCED_VERSION, LAST_SYNCED_TIMESTAMP, FILE
    from FILES where PATH=? and NAME=?";

static SQL_DELETE_SYNCHRO: &str = "update FILES 
    set HASH=null, IS_SYNCED=true, LAST_SYNCED_VERSION=?, LAST_SYNCED_TIMESTAMP=?, FILE=null
    where PATH=? and NAME=?";

static SQL_DELETE_NOT_SYNCHRO: &str = "update FILES 
    set HASH=null, IS_SYNCED=false, FILE=null
    where PATH=? and NAME=?";

static SQL_UPDATE_SYNCHRO: &str = "insert into FILES
    (PATH, NAME, HASH, IS_SYNCED, LAST_SYNCED_VERSION, LAST_SYNCED_TIMESTAMP, FILE)
    values (?, ?, ?, true, ?, ?, ?)
    on conflict(PATH, NAME) do update
    set HASH=excluded.HASH, IS_SYNCED=true, LAST_SYNCED_VERSION=excluded.LAST_SYNCED_VERSION, LAST_SYNCED_TIMESTAMP=excluded.LAST_SYNCED_TIMESTAMP, FILE=excluded.FILE";

static SQL_UPDATE_NOT_SYNCHRO: &str = "insert into FILES
    (PATH, NAME, HASH, IS_SYNCED, LAST_SYNCED_VERSION, LAST_SYNCED_TIMESTAMP, FILE)
    values (?, ?, ?, false, null, null, ?)
    on conflict(PATH, NAME) do update
    set HASH=excluded.HASH, IS_SYNCED=true, FILE=excluded.FILE";

pub struct CacheDb {
    connection: rusqlite::Connection,
}

pub struct SyncInformation {
    pub last_synced_version: i32,
    pub last_synced_timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct CacheDbFile {
    pub hash: String,
    pub is_synced: bool,
    pub last_synced_version: Option<i32>,
    pub last_synced_timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub file: Option<Vec<u8>>,
}

impl CacheDb {
    pub fn new<T>(root_path: T) -> Result<CacheDb, router::RouterError>
    where
        T: std::convert::Into<std::path::PathBuf>,
    {
        let root_path: std::path::PathBuf = root_path.into();
        if root_path.exists() && !root_path.is_dir() {
            log::error!("Path {:?} exists and is not a directory.", root_path);
            return Err(router::RouterError::HandlerError(
                500,
                String::from("File cache path exists and is not a directory"),
            ));
        }
        if !root_path.exists() {
            log::info!("Creating path: {:?}", root_path);
            std::fs::create_dir_all(&root_path).map_err(|e| {
                router::RouterError::HandlerError(
                    500,
                    format!("Could not create directory for file cache: {}", e),
                )
            })?;
        }
        let db_path = root_path.join("file_cache.db3");
        if db_path.exists() {
            log::info!("Database already exists")
        }

        log::info!("Opening database in {:?}", db_path);
        let connection = rusqlite::Connection::open(db_path);
        let connection = map_sqlite_result(connection, "Failed to open sqlite database")?;

        let result = connection.execute(SQL_CREATE_FILES_TABLE, []);
        map_sqlite_result(result, "Failed to create FILES table in sqlite database")?;

        Ok(CacheDb { connection })
    }

    pub fn get(
        &self,
        file_path: &str,
        file_name: &str,
    ) -> Result<CacheDbFile, router::RouterError> {
        // TODO check if we have a file
        map_sqlite_result(self.get_inner(file_path, file_name), "Failed to ")
    }

    pub fn delete(
        &self,
        file_path: &str,
        file_name: &str,
        sync_information: Option<SyncInformation>,
    ) -> Result<(), router::RouterError> {
        let n_deleted_rows = if let Some(sync_information) = sync_information {
            map_sqlite_result(
                self.delete_inner_synchro(
                    file_path,
                    file_name,
                    sync_information.last_synced_version,
                    sync_information.last_synced_timestamp,
                ),
                "Failed to delete file",
            )?
        } else {
            map_sqlite_result(
                self.delete_inner_not_synchro(file_path, file_name),
                "Failed to delete file",
            )?
        };
        if n_deleted_rows == 0 {
            Err(router::RouterError::NotFound)
        } else {
            Ok(())
        }
    }

    pub fn save(
        &self,
        file_path: &str,
        file_name: &str,
        sync_information: Option<SyncInformation>,
        file: &Vec<u8>,
    ) -> Result<(), router::RouterError> {
        let hash = digest(file);
        let n_updated_rows = if let Some(sync_information) = sync_information {
            map_sqlite_result(
                self.save_inner_synchro(
                    file_path,
                    file_name,
                    hash,
                    sync_information.last_synced_version,
                    sync_information.last_synced_timestamp,
                    file,
                ),
                "Failed to update the file",
            )?
        } else {
            map_sqlite_result(
                self.save_inner_not_synchro(file_path, file_name, hash, file),
                "Failed to update the file",
            )?
        };
        if n_updated_rows == 0 {
            Err(router::RouterError::HandlerError(
                500,
                String::from("Failed to update the file"),
            ))
        } else {
            Ok(())
        }
    }

    fn get_inner(&self, file_path: &str, file_name: &str) -> Result<CacheDbFile, rusqlite::Error> {
        self.connection.query_row(
            SQL_SELECT_FILE,
            rusqlite::params![file_path, file_name],
            |row| {
                let timestamp: Option<String> = row.get(3)?;
                let timestamp = timestamp.and_then(|e| decode_timestamp(e).ok());
                Ok(CacheDbFile {
                    hash: row.get(0)?,
                    is_synced: row.get(1)?,
                    last_synced_version: row.get(2)?,
                    last_synced_timestamp: timestamp,
                    file: row.get(4)?,
                })
            },
        )
    }

    fn delete_inner_synchro(
        &self,
        file_path: &str,
        file_name: &str,
        version: i32,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> Result<usize, rusqlite::Error> {
        self.connection.execute(
            SQL_DELETE_SYNCHRO,
            rusqlite::params![version, timestamp.to_rfc3339(), file_path, file_name],
        )
    }

    fn delete_inner_not_synchro(
        &self,
        file_path: &str,
        file_name: &str,
    ) -> Result<usize, rusqlite::Error> {
        self.connection.execute(
            SQL_DELETE_NOT_SYNCHRO,
            rusqlite::params![file_path, file_name],
        )
    }

    fn save_inner_synchro(
        &self,
        file_path: &str,
        file_name: &str,
        hash: String,
        version: i32,
        timestamp: chrono::DateTime<chrono::Utc>,
        file: &Vec<u8>,
    ) -> Result<usize, rusqlite::Error> {
        self.connection.execute(
            SQL_UPDATE_SYNCHRO,
            rusqlite::params![
                file_path,
                file_name,
                &hash,
                version,
                timestamp.to_rfc3339(),
                file
            ],
        )
    }

    fn save_inner_not_synchro(
        &self,
        file_path: &str,
        file_name: &str,
        hash: String,
        file: &Vec<u8>,
    ) -> Result<usize, rusqlite::Error> {
        self.connection.execute(
            SQL_UPDATE_NOT_SYNCHRO,
            rusqlite::params![file_path, file_name, &hash, file],
        )
    }
}

fn decode_timestamp(timestamp: String) -> Result<chrono::DateTime<chrono::Utc>, rusqlite::Error> {
    chrono::DateTime::parse_from_rfc3339(timestamp.as_ref())
        .map(|ts| ts.with_timezone(&chrono::Utc))
        .map_err(|_| rusqlite::Error::InvalidColumnName(String::from("Failed to decode timestamp")))
}

fn digest(data: &Vec<u8>) -> String {
    base64::encode(sha2::Sha256::digest(&data).to_vec())
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
