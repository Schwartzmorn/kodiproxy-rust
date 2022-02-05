#[derive(PartialEq)]
enum LastEvent {
    None,
    Deletion(String, chrono::DateTime<chrono::Utc>),
    Update(String, chrono::DateTime<chrono::Utc>),
}

#[derive(Debug, PartialEq)]
pub enum ComparisonResult {
    Equal,
    LocalIsMoreRecent,
    DistantIsMoreRecent,
    Diverge,
}

// returns a simplified view of the last operation that happened to the file
fn get_last_event(log: &Option<files::log::FileLog>) -> LastEvent {
    match log {
        Some(log) => {
            if log.entries.len() == 0 {
                return LastEvent::None;
            }
            let datetime = log.entries.last().unwrap().timestamp;
            match &log.entries.last().unwrap().entry {
                files::log::FileLogEntryType::Creation { hash, .. }
                | files::log::FileLogEntryType::Modification { hash, .. }
                | files::log::FileLogEntryType::MoveFrom { hash, .. } => {
                    LastEvent::Update(hash.to_owned(), datetime)
                }
                _ => {
                    let last_update = log.entries.iter().rev().find(|&entry| match entry.entry {
                        files::log::FileLogEntryType::MoveFrom { .. }
                        | files::log::FileLogEntryType::Modification { .. }
                        | files::log::FileLogEntryType::Creation { .. } => true,
                        _ => false,
                    });
                    return match last_update {
                        Some(entry) => match &entry.entry {
                            files::log::FileLogEntryType::MoveFrom { hash, .. }
                            | files::log::FileLogEntryType::Modification { hash, .. }
                            | files::log::FileLogEntryType::Creation { hash, .. } => {
                                LastEvent::Deletion(hash.to_owned(), datetime)
                            }
                            _ => LastEvent::None,
                        },
                        None => LastEvent::None,
                    };
                }
            }
        }
        None => LastEvent::None,
    }
}

fn explode_event(event: &LastEvent) -> (&String, chrono::DateTime<chrono::Utc>) {
    match event {
        LastEvent::None => panic!(),
        LastEvent::Deletion(hash, datetime) => (
            hash,
            datetime
                .clone()
                .checked_add_signed(chrono::Duration::seconds(10))
                .unwrap_or(datetime.clone()),
        ),
        LastEvent::Update(hash, datetime) => (hash, datetime.clone()),
    }
}

fn is_hash_in_log(hash_in: &String, log: &files::log::FileLog) -> bool {
    let res = log.entries.iter().rev().find(|&entry| match &entry.entry {
        files::log::FileLogEntryType::MoveFrom { hash, .. }
        | files::log::FileLogEntryType::Modification { hash, .. }
        | files::log::FileLogEntryType::Creation { hash, .. } => hash == hash_in,
        _ => false,
    });
    match res {
        Some(..) => true,
        None => false,
    }
}

pub fn compare_logs(
    local_log: &Option<files::log::FileLog>,
    distant_log: &Option<files::log::FileLog>,
) -> ComparisonResult {
    let last_local_event = get_last_event(local_log);
    let last_distant_event = get_last_event(distant_log);

    if last_local_event == last_distant_event {
        return ComparisonResult::Equal;
    }
    if last_local_event == LastEvent::None {
        return ComparisonResult::DistantIsMoreRecent;
    }
    if last_distant_event == LastEvent::None {
        return ComparisonResult::LocalIsMoreRecent;
    }
    let (last_local_hash, last_local_datetime) = explode_event(&last_local_event);
    let (last_distant_hash, last_distant_datetime) = explode_event(&last_distant_event);
    if last_local_hash == last_distant_hash {
        return if last_local_datetime < last_distant_datetime {
            ComparisonResult::DistantIsMoreRecent
        } else {
            ComparisonResult::LocalIsMoreRecent
        };
    }

    if is_hash_in_log(last_local_hash, &distant_log.as_ref().unwrap()) {
        return ComparisonResult::DistantIsMoreRecent;
    } else if is_hash_in_log(last_distant_hash, &local_log.as_ref().unwrap()) {
        return ComparisonResult::LocalIsMoreRecent;
    }

    return ComparisonResult::Diverge;
}

#[cfg(test)]
mod test {
    use super::*;

    static LOG_DELETION_A: &str = r#"2021-01-01T00:00:00.00000000Z [127.0.0.1] Creation[0:HASH_A]
2021-01-02T00:00:00.00000000Z [127.0.0.1] Deletion[]"#;
    static LOG_CREATION_A: &str = r#"2021-01-01T00:00:00.00000000Z [127.0.0.1] Creation[0:HASH_A]"#;
    static LOG_MODIFICATION_B: &str = r#"2021-01-01T00:00:00.00000000Z [127.0.0.1] Creation[0:HASH_A]
2021-01-02T00:00:00.00000000Z [127.0.0.1] Modification[1:HASH_B]"#;
    static LOG_MODIFICATION_C: &str = r#"2021-01-01T00:00:00.00000000Z [127.0.0.1] Creation[0:HASH_A]
2021-01-02T00:00:00.00000000Z [127.0.0.1] Modification[1:HASH_C]"#;

    fn to_log(log: &str) -> Option<files::log::FileLog> {
        if log == "" {
            Option::None
        } else {
            Some(files::log::FileLog::new_from_str(log))
        }
    }

    fn compare(local: &str, distant: &str) -> ComparisonResult {
        compare_logs(&to_log(local), &to_log(distant))
    }

    #[test]
    fn it_handles_simple_cases() {
        assert_eq!(
            compare(LOG_CREATION_A, ""),
            ComparisonResult::LocalIsMoreRecent
        );
        assert_eq!(
            compare("", LOG_CREATION_A),
            ComparisonResult::DistantIsMoreRecent
        );

        assert_eq!(
            compare(LOG_CREATION_A, LOG_DELETION_A),
            ComparisonResult::DistantIsMoreRecent
        );
        assert_eq!(
            compare(LOG_DELETION_A, LOG_CREATION_A),
            ComparisonResult::LocalIsMoreRecent
        );

        assert_eq!(
            compare(LOG_CREATION_A, LOG_MODIFICATION_B),
            ComparisonResult::DistantIsMoreRecent
        );
        assert_eq!(
            compare(LOG_MODIFICATION_B, LOG_CREATION_A),
            ComparisonResult::LocalIsMoreRecent
        );

        assert_eq!(
            compare(LOG_MODIFICATION_B, LOG_MODIFICATION_C),
            ComparisonResult::Diverge
        );
    }
}
