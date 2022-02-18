pub struct FileLog {
    pub entries: Vec<FileLogEntry>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type")]
pub enum FileLogEntryType {
    Creation {
        version: u32,
        hash: String,
    },
    Deletion {
        version: u32,
    },
    Update {
        version: u32,
        hash: String,
    },
    MoveTo {
        version: u32,
        #[serde(rename = "pathTo")]
        path_to: std::path::PathBuf,
    },
    MoveFrom {
        version: u32,
        hash: String,
        #[serde(rename = "pathFrom")]
        path_from: std::path::PathBuf,
    },
}

impl FileLogEntryType {
    pub fn new(
        entry_type: String,
        version: u32,
        hash: Option<String>,
        path: Option<String>,
    ) -> Result<FileLogEntryType, String> {
        match entry_type.as_ref() {
            "CREATION" => {
                if let Some(hash) = hash {
                    Ok(FileLogEntryType::Creation { version, hash })
                } else {
                    Err(String::from("No hash given for a Creation entry"))
                }
            }
            "DELETION" => Ok(FileLogEntryType::Deletion { version }),
            "MOVE_FROM" => {
                if let (Some(hash), Some(path)) = (hash, path) {
                    Ok(FileLogEntryType::MoveFrom {
                        version,
                        hash,
                        path_from: std::path::PathBuf::from(path),
                    })
                } else {
                    Err(String::from("Hash or path not given for a MoveFrom entry"))
                }
            }
            "MOVE_TO" => {
                if let Some(path) = path {
                    Ok(FileLogEntryType::MoveTo {
                        version,
                        path_to: std::path::PathBuf::from(path),
                    })
                } else {
                    Err(String::from("No path given for a MoveTo entry"))
                }
            }
            "UPDATE" => {
                if let Some(hash) = hash {
                    Ok(FileLogEntryType::Update { version, hash })
                } else {
                    Err(String::from("No hash given for an Update entry"))
                }
            }
            _ => Err(format!("Unknown entry type: {}", entry_type)),
        }
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct FileLogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub address: std::net::IpAddr,
    pub entry: FileLogEntryType,
}
