//! Domain types and parsers for line blame and path history.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlameLine {
    pub line: usize,
    pub commit_oid: String,
    pub author_name: String,
    pub author_email: String,
    pub authored_at: i64,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBlame {
    pub path: Vec<u8>,
    pub revision: Option<String>,
    pub lines: Vec<BlameLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathHistoryEntry {
    pub oid: String,
    pub parent_oid: Option<String>,
    pub author_name: String,
    pub author_email: String,
    pub authored_at: i64,
    pub subject: String,
    pub path: Vec<u8>,
    pub previous_path: Option<Vec<u8>>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathHistory {
    pub path: Vec<u8>,
    pub is_directory: bool,
    pub entries: Vec<PathHistoryEntry>,
    pub next_cursor: Option<String>,
}

pub fn parse_blame_porcelain(output: &[u8]) -> Result<Vec<BlameLine>, String> {
    let text = String::from_utf8_lossy(output);
    let mut current_oid = String::new();
    let mut author_name = String::new();
    let mut author_email = String::new();
    let mut authored_at = 0_i64;
    let mut next_line = 1_usize;
    let mut remaining = 0_usize;
    let mut lines = Vec::new();

    for raw in text.lines() {
        if let Some(content) = raw.strip_prefix('\t') {
            if current_oid.is_empty() {
                return Err("blame content was not preceded by a header".to_owned());
            }
            lines.push(BlameLine {
                line: next_line,
                commit_oid: current_oid.clone(),
                author_name: author_name.clone(),
                author_email: author_email.clone(),
                authored_at,
                content: content.to_owned(),
            });
            next_line += 1;
            remaining = remaining.saturating_sub(1);
            continue;
        }

        let fields = raw.split_whitespace().collect::<Vec<_>>();
        if fields.len() >= 4
            && fields[0].len() >= 7
            && fields[1].parse::<usize>().is_ok()
            && fields[2].parse::<usize>().is_ok()
        {
            current_oid = fields[0].to_owned();
            next_line = fields[2]
                .parse::<usize>()
                .map_err(|_| "invalid blame line".to_owned())?;
            remaining = fields[3]
                .parse::<usize>()
                .map_err(|_| "invalid blame group size".to_owned())?;
            continue;
        }
        if let Some(value) = raw.strip_prefix("author ") {
            author_name = value.to_owned();
        } else if let Some(value) = raw.strip_prefix("author-mail ") {
            author_email = value.trim_matches(['<', '>']).to_owned();
        } else if let Some(value) = raw.strip_prefix("author-time ") {
            authored_at = value
                .trim()
                .parse()
                .map_err(|_| "invalid blame timestamp".to_owned())?;
        }
    }

    if remaining != 0 {
        return Err("blame record was incomplete".to_owned());
    }
    Ok(lines)
}

pub fn parse_path_history(output: &[u8]) -> Result<Vec<PathHistoryEntry>, String> {
    let mut entries = Vec::new();
    for record in output.split(|byte| *byte == 0x1e) {
        let mut record = record;
        while record
            .first()
            .is_some_and(|byte| *byte == b'\n' || *byte == b'\r')
        {
            record = &record[1..];
        }
        if record.iter().all(|byte| byte.is_ascii_whitespace()) {
            continue;
        }
        let fields = record.split(|byte| *byte == 0).collect::<Vec<_>>();
        if fields.len() < 6 {
            return Err("path history record is incomplete".to_owned());
        }
        let text = |field: &[u8]| String::from_utf8_lossy(field).trim().to_owned();
        let oid = text(fields[0]);
        if oid.is_empty() {
            continue;
        }
        let parents = text(fields[1]);
        let authored_at = text(fields[4])
            .parse::<i64>()
            .map_err(|_| "path history timestamp is invalid".to_owned())?;
        let mut status = String::new();
        let mut path = Vec::new();
        let mut previous_path = None;
        let mut index = 6;
        if fields.len() == 6 {
            return Err("path history record is incomplete".to_owned());
        }
        if fields.len() == 7 {
            let status_blob = String::from_utf8_lossy(fields[6]);
            let status_line = status_blob
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or_default();
            let mut status_parts = status_line.splitn(2, '\t');
            status = status_parts.next().unwrap_or_default().to_owned();
            path = status_parts.next().unwrap_or_default().as_bytes().to_vec();
            if status.starts_with('R') || status.starts_with('C') {
                let parts = status_line.split('\t').collect::<Vec<_>>();
                if parts.len() < 3 {
                    return Err("rename path history record is incomplete".to_owned());
                }
                previous_path = Some(parts[1].as_bytes().to_vec());
                path = parts[2].as_bytes().to_vec();
            }
            index = fields.len();
        }
        while index < fields.len() {
            if fields[index].is_empty() {
                index += 1;
                continue;
            }
            let status_bytes = fields[index];
            status = text(status_bytes);
            index += 1;
            if status.starts_with('R') || status.starts_with('C') {
                if index + 1 >= fields.len() {
                    return Err("rename path history record is incomplete".to_owned());
                }
                previous_path = Some(fields[index].to_vec());
                path = fields[index + 1].to_vec();
            } else if index < fields.len() {
                path = fields[index].to_vec();
            }
            break;
        }
        if path.is_empty() {
            path = fields
                .get(6)
                .map(|field| field.to_vec())
                .unwrap_or_default();
        }
        entries.push(PathHistoryEntry {
            oid,
            parent_oid: parents.split_whitespace().next().map(str::to_owned),
            author_name: text(fields[2]),
            author_email: text(fields[3]),
            authored_at,
            subject: text(fields[5]),
            path,
            previous_path,
            status,
        });
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::{parse_blame_porcelain, parse_path_history};

    #[test]
    fn parses_blame_groups_and_line_metadata() {
        let oid = "a".repeat(40);
        let output = format!(
            "{oid} 1 1 2\nauthor Alice\nauthor-mail <alice@example.com>\nauthor-time 42\n\tfirst\n\tsecond\n"
        );
        let lines = parse_blame_porcelain(output.as_bytes()).expect("blame");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].line, 2);
        assert_eq!(lines[0].author_email, "alice@example.com");
    }

    #[test]
    fn parses_rename_path_history_records() {
        let oid = "b".repeat(40);
        let parent = "c".repeat(40);
        let output = format!(
            "{oid}\0{parent}\0Alice\0alice@example.com\042\0Rename\0R100\0old.txt\0new.txt\0\x1e\n"
        );
        let entries = parse_path_history(output.as_bytes()).expect("history");
        assert_eq!(
            entries[0].previous_path.as_deref(),
            Some(b"old.txt".as_slice())
        );
        assert_eq!(entries[0].path, b"new.txt");
    }
}
