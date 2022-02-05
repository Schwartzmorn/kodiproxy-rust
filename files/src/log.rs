use std::{io::BufRead, str::FromStr};

pub struct FileLogWriter {
    log_file: std::fs::File,
}

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
    Deletion,
    Modification {
        version: u32,
        hash: String,
    },
    MoveTo {
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

#[derive(serde::Deserialize, serde::Serialize)]
pub struct FileLogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub address: std::net::IpAddr,
    pub entry: FileLogEntryType,
}

impl FileLogWriter {
    pub fn new(log_path: std::path::PathBuf) -> Result<FileLogWriter, std::io::Error> {
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path.join("manifest"))?;
        Ok(FileLogWriter { log_file })
    }

    pub fn append(&mut self, entry_type: FileLogEntryType) -> Result<(), std::io::Error> {
        let entry = FileLogEntry {
            entry: entry_type,
            timestamp: chrono::Utc::now(),
            address: [127, 0, 0, 1].into(), // TODO change this
        };
        entry.encode(&mut self.log_file)
    }
}

impl FileLogEntryType {
    pub fn encode(&self, writer: &mut dyn std::io::Write) -> Result<(), std::io::Error> {
        match &self {
            FileLogEntryType::Creation { version, hash } => {
                write!(writer, "Creation[{}:{}]", version, hash)
            }
            FileLogEntryType::Deletion => {
                write!(writer, "Deletion[]")
            }
            FileLogEntryType::Modification { version, hash } => {
                write!(writer, "Modification[{}:{}]", version, hash)
            }
            FileLogEntryType::MoveTo { path_to } => {
                write!(writer, "MoveTo[::{}]", path_to.to_string_lossy())
            }
            FileLogEntryType::MoveFrom {
                version,
                hash,
                path_from,
            } => write!(
                writer,
                "MoveFrom[{}:{}:{}]",
                version,
                hash,
                path_from.to_string_lossy()
            ),
        }
    }

    pub fn decode(line: &str) -> Option<FileLogEntryType> {
        lazy_static::lazy_static! {
            static ref RE: regex::Regex = regex::Regex::new(r"^(?P<type>[A-z]+)\[(?P<values>.*)\]").unwrap();
        }
        lazy_static::lazy_static! {
            static ref RE_VALUES: regex::Regex = regex::Regex::new(r"^((?P<version>[^:]*):(?P<hash>[^:]*)(:(?P<path>.+))?)?$").unwrap();
        }

        RE.captures(line)
            .map(|captures| {
                let values = RE_VALUES.captures(&captures["values"])?;

                let version = values
                    .name("version")
                    .map(|s| s.as_str().parse::<u32>().ok())
                    .flatten();
                let hash = values.name("hash").map(|m| m.as_str().to_owned());
                let path = values
                    .name("path")
                    .map(|m| std::path::PathBuf::from(m.as_str()));

                match &captures["type"] {
                    "Creation" => {
                        if let (Some(version), Some(hash)) = (version, hash) {
                            Some(FileLogEntryType::Creation { version, hash })
                        } else {
                            None
                        }
                    }
                    "Modification" => {
                        if let (Some(version), Some(hash)) = (version, hash) {
                            Some(FileLogEntryType::Modification { version, hash })
                        } else {
                            None
                        }
                    }
                    "Deletion" => Some(FileLogEntryType::Deletion),
                    "MoveTo" => {
                        let path_to = values
                            .name("path")
                            .map(|m| std::path::PathBuf::from(m.as_str()))?;
                        Some(FileLogEntryType::MoveTo { path_to })
                    }
                    "MoveFrom" => {
                        if let (Some(version), Some(hash), Some(path_from)) = (version, hash, path)
                        {
                            Some(FileLogEntryType::MoveFrom {
                                version,
                                hash,
                                path_from,
                            })
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            })
            .flatten()
    }
}

impl FileLogEntry {
    pub fn encode(&self, writer: &mut dyn std::io::Write) -> Result<(), std::io::Error> {
        write!(writer, "{:?} [{:?}] ", self.timestamp, self.address)?;
        self.entry.encode(writer)?;
        write!(writer, "\n")?;
        Ok(())
    }

    pub fn decode(line: &str) -> Option<FileLogEntry> {
        lazy_static::lazy_static! {
            static ref RE: regex::Regex = regex::Regex::new(r"^\s*(?P<timestamp>[^ ]+)\s+\[(?P<address>[^\]]+)\]\s+(?P<entry_type>.+\])").unwrap();
        }
        let res = RE.captures(&line).and_then(|captures| {
            let timestamp = chrono::DateTime::parse_from_rfc3339(&captures["timestamp"])
                .map(|ts| ts.with_timezone(&chrono::Utc))
                .ok()?;
            let address = std::net::IpAddr::from_str(&captures["address"]).ok()?;

            let entry_type = FileLogEntryType::decode(&captures["entry_type"])?;

            Some(FileLogEntry {
                timestamp,
                address,
                entry: entry_type,
            })
        });
        if res.is_none() {
            log::debug!("Could not parse line in log file '{}'", line);
        }
        res
    }
}

impl FileLog {
    pub fn new(log_path: std::path::PathBuf) -> Result<FileLog, std::io::Error> {
        log::debug!("Reading manifest '{:?}'", log_path);
        let log_file = std::fs::File::open(log_path.join("manifest"))?;
        let entries = std::io::BufReader::new(log_file)
            .lines()
            .inspect(|line| {
                if let Err(e) = line {
                    log::debug!("Could not read line in log file: {}", e);
                }
            })
            .filter(Result::is_ok)
            .map(Result::unwrap)
            .filter(|s| !s.is_empty())
            .map(|s| FileLogEntry::decode(s.as_str()))
            .filter(Option::is_some)
            .map(Option::unwrap)
            .collect::<Vec<_>>();
        Ok(FileLog { entries })
    }

    pub fn new_from_str(log_str: &str) -> FileLog {
        FileLog {
            entries: log_str
                .split('\n')
                .map(FileLogEntry::decode)
                .filter(Option::is_some)
                .map(Option::unwrap)
                .collect::<Vec<_>>(),
        }
    }
}
