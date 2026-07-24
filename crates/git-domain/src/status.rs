use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusSnapshot {
    pub head: HeadState,
    pub upstream: Option<String>,
    pub ahead: u64,
    pub behind: u64,
    pub stash_count: u64,
    pub changes: Vec<FileChange>,
}

impl Default for StatusSnapshot {
    fn default() -> Self {
        Self {
            head: HeadState::Unborn,
            upstream: None,
            ahead: 0,
            behind: 0,
            stash_count: 0,
            changes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadState {
    Unborn,
    Detached { oid: Option<String> },
    Branch { name: String, oid: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: Vec<u8>,
    pub original_path: Option<Vec<u8>>,
    pub index_status: u8,
    pub worktree_status: u8,
    pub is_conflict: bool,
    pub is_submodule: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusParseError {
    record: Vec<u8>,
    reason: &'static str,
}

impl StatusParseError {
    fn new(record: &[u8], reason: &'static str) -> Self {
        Self {
            record: record.to_vec(),
            reason,
        }
    }
}

impl fmt::Display for StatusParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {:?}",
            self.reason,
            String::from_utf8_lossy(&self.record)
        )
    }
}

impl std::error::Error for StatusParseError {}

pub fn parse_porcelain_v2(output: &[u8]) -> Result<StatusSnapshot, StatusParseError> {
    let records: Vec<&[u8]> = output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .collect();
    let mut snapshot = StatusSnapshot::default();
    let mut branch_name = None;
    let mut branch_oid = None;
    let mut branch_initial = false;
    let mut index = 0;

    while index < records.len() {
        let record = records[index];
        match record.first().copied() {
            Some(b'#') => parse_header(
                record,
                &mut snapshot,
                &mut branch_name,
                &mut branch_oid,
                &mut branch_initial,
            )?,
            Some(b'1') => snapshot.changes.push(parse_ordinary(record)?),
            Some(b'2') => {
                let original_path = records.get(index + 1).ok_or_else(|| {
                    StatusParseError::new(record, "rename is missing original path")
                })?;
                snapshot.changes.push(parse_rename(record, original_path)?);
                index += 1;
            }
            Some(b'u') => snapshot.changes.push(parse_unmerged(record)?),
            Some(b'?') => snapshot.changes.push(parse_untracked(record)?),
            Some(b'!') => {}
            _ => return Err(StatusParseError::new(record, "unknown status record")),
        }
        index += 1;
    }

    snapshot.head = match (branch_initial, branch_name.as_deref()) {
        (true, _) => HeadState::Unborn,
        (false, Some("(detached)")) => HeadState::Detached { oid: branch_oid },
        (false, Some(name)) => HeadState::Branch {
            name: name.to_owned(),
            oid: branch_oid,
        },
        (false, None) => HeadState::Unborn,
    };

    Ok(snapshot)
}

fn parse_header(
    record: &[u8],
    snapshot: &mut StatusSnapshot,
    branch_name: &mut Option<String>,
    branch_oid: &mut Option<String>,
    branch_initial: &mut bool,
) -> Result<(), StatusParseError> {
    let text = std::str::from_utf8(record)
        .map_err(|_| StatusParseError::new(record, "header is not UTF-8"))?;

    if let Some(value) = text.strip_prefix("# branch.oid ") {
        if value == "(initial)" {
            *branch_initial = true;
        } else {
            *branch_oid = Some(value.to_owned());
        }
    } else if let Some(value) = text.strip_prefix("# branch.head ") {
        *branch_name = Some(value.to_owned());
    } else if let Some(value) = text.strip_prefix("# branch.upstream ") {
        snapshot.upstream = Some(value.to_owned());
    } else if let Some(value) = text.strip_prefix("# branch.ab ") {
        let mut values = value.split_whitespace();
        snapshot.ahead = parse_count(values.next(), '+', record)?;
        snapshot.behind = parse_count(values.next(), '-', record)?;
    } else if let Some(value) = text.strip_prefix("# stash ") {
        snapshot.stash_count = value
            .parse()
            .map_err(|_| StatusParseError::new(record, "invalid stash count"))?;
    }

    Ok(())
}

fn parse_count(value: Option<&str>, prefix: char, record: &[u8]) -> Result<u64, StatusParseError> {
    value
        .and_then(|item| item.strip_prefix(prefix))
        .and_then(|item| item.parse().ok())
        .ok_or_else(|| StatusParseError::new(record, "invalid ahead/behind header"))
}

fn parse_ordinary(record: &[u8]) -> Result<FileChange, StatusParseError> {
    let fields: Vec<&[u8]> = record.splitn(9, |byte| *byte == b' ').collect();
    if fields.len() != 9 {
        return Err(StatusParseError::new(record, "invalid ordinary record"));
    }
    change_from_fields(fields[1], fields[2], fields[8], None, false, record)
}

fn parse_rename(record: &[u8], original_path: &[u8]) -> Result<FileChange, StatusParseError> {
    let fields: Vec<&[u8]> = record.splitn(10, |byte| *byte == b' ').collect();
    if fields.len() != 10 {
        return Err(StatusParseError::new(record, "invalid rename record"));
    }
    change_from_fields(
        fields[1],
        fields[2],
        fields[9],
        Some(original_path.to_vec()),
        false,
        record,
    )
}

fn parse_unmerged(record: &[u8]) -> Result<FileChange, StatusParseError> {
    let fields: Vec<&[u8]> = record.splitn(11, |byte| *byte == b' ').collect();
    if fields.len() != 11 {
        return Err(StatusParseError::new(record, "invalid unmerged record"));
    }
    change_from_fields(fields[1], fields[2], fields[10], None, true, record)
}

fn parse_untracked(record: &[u8]) -> Result<FileChange, StatusParseError> {
    let path = record
        .strip_prefix(b"? ")
        .ok_or_else(|| StatusParseError::new(record, "invalid untracked record"))?;
    Ok(FileChange {
        path: path.to_vec(),
        original_path: None,
        index_status: b'?',
        worktree_status: b'?',
        is_conflict: false,
        is_submodule: false,
    })
}

fn change_from_fields(
    xy: &[u8],
    submodule: &[u8],
    path: &[u8],
    original_path: Option<Vec<u8>>,
    is_conflict: bool,
    record: &[u8],
) -> Result<FileChange, StatusParseError> {
    if xy.len() != 2 {
        return Err(StatusParseError::new(record, "invalid XY status"));
    }
    Ok(FileChange {
        path: path.to_vec(),
        original_path,
        index_status: xy[0],
        worktree_status: xy[1],
        is_conflict,
        is_submodule: submodule.first() == Some(&b'S'),
    })
}

#[cfg(test)]
mod tests {
    use super::{HeadState, parse_porcelain_v2};

    #[test]
    fn parses_headers_and_byte_safe_paths() {
        let output = concat!(
            "# branch.oid abcdef\0",
            "# branch.head main\0",
            "# branch.upstream origin/main\0",
            "# branch.ab +2 -1\0",
            "# stash 3\0",
            "1 M. N... 100644 100644 100644 abc def staged file.txt\0",
            "? line\nbreak.txt\0",
        );

        let snapshot = parse_porcelain_v2(output.as_bytes()).expect("valid status");

        assert_eq!(
            snapshot.head,
            HeadState::Branch {
                name: "main".to_owned(),
                oid: Some("abcdef".to_owned())
            }
        );
        assert_eq!(snapshot.ahead, 2);
        assert_eq!(snapshot.behind, 1);
        assert_eq!(snapshot.stash_count, 3);
        assert_eq!(snapshot.changes[1].path, b"line\nbreak.txt");
    }

    #[test]
    fn parses_rename_with_original_path_in_following_record() {
        let output = b"2 R. N... 100644 100644 100644 abc def R100 new name.txt\0old name.txt\0";

        let snapshot = parse_porcelain_v2(output).expect("valid rename");

        assert_eq!(snapshot.changes[0].path, b"new name.txt");
        assert_eq!(
            snapshot.changes[0].original_path.as_deref(),
            Some(b"old name.txt".as_slice())
        );
    }

    #[test]
    fn rejects_truncated_record() {
        let error = parse_porcelain_v2(b"1 M. broken\0").expect_err("invalid status");
        assert!(error.to_string().contains("ordinary"));
    }

    #[test]
    fn recognizes_unborn_branch() {
        let output = b"# branch.oid (initial)\0# branch.head main\0";
        let snapshot = parse_porcelain_v2(output).expect("valid unborn status");
        assert_eq!(snapshot.head, HeadState::Unborn);
    }
}
