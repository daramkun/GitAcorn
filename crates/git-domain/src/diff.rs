use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiffDocument {
    pub files: Vec<DiffFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffFile {
    pub old_path: String,
    pub new_path: String,
    pub header: Vec<u8>,
    pub hunks: Vec<DiffHunk>,
    pub binary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    pub index: usize,
    pub old_start: u64,
    pub old_count: u64,
    pub new_start: u64,
    pub new_count: u64,
    pub header: String,
    pub raw: Vec<u8>,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Addition,
    Deletion,
    NoNewline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_line: Option<u64>,
    pub new_line: Option<u64>,
    pub content: String,
    pub raw_content: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffParseError {
    reason: String,
}

impl DiffParseError {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl fmt::Display for DiffParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.reason.fmt(formatter)
    }
}

impl std::error::Error for DiffParseError {}

pub fn parse_unified_diff(output: &[u8]) -> Result<DiffDocument, DiffParseError> {
    if output.is_empty() {
        return Ok(DiffDocument::default());
    }

    let lines = split_inclusive_lines(output);
    let mut document = DiffDocument::default();
    let mut index = 0;

    while index < lines.len() {
        if !lines[index].starts_with(b"diff --git ") {
            index += 1;
            continue;
        }
        let file_start = index;
        index += 1;
        while index < lines.len()
            && !lines[index].starts_with(b"@@ ")
            && !lines[index].starts_with(b"diff --git ")
        {
            index += 1;
        }
        let header = lines[file_start..index].concat();
        let (old_path, new_path) = parse_paths(&header)?;
        let binary = header
            .windows(b"Binary files ".len())
            .any(|window| window == b"Binary files ");
        let mut hunks = Vec::new();

        while index < lines.len() && !lines[index].starts_with(b"diff --git ") {
            if !lines[index].starts_with(b"@@ ") {
                index += 1;
                continue;
            }
            let hunk_start = index;
            let (old_start, old_count, new_start, new_count, hunk_header) =
                parse_hunk_header(lines[index])?;
            index += 1;
            let mut old_line = old_start;
            let mut new_line = new_start;
            let mut parsed_lines = Vec::new();
            while index < lines.len()
                && !lines[index].starts_with(b"@@ ")
                && !lines[index].starts_with(b"diff --git ")
            {
                let line = lines[index];
                let raw_content = trim_line_feed(&line[1..]).to_vec();
                let display_content = raw_content.strip_suffix(b"\r").unwrap_or(&raw_content);
                let content = String::from_utf8_lossy(display_content).into_owned();
                match line.first().copied() {
                    Some(b' ') => {
                        parsed_lines.push(DiffLine {
                            kind: DiffLineKind::Context,
                            old_line: Some(old_line),
                            new_line: Some(new_line),
                            content,
                            raw_content,
                        });
                        old_line += 1;
                        new_line += 1;
                    }
                    Some(b'+') => {
                        parsed_lines.push(DiffLine {
                            kind: DiffLineKind::Addition,
                            old_line: None,
                            new_line: Some(new_line),
                            content,
                            raw_content,
                        });
                        new_line += 1;
                    }
                    Some(b'-') => {
                        parsed_lines.push(DiffLine {
                            kind: DiffLineKind::Deletion,
                            old_line: Some(old_line),
                            new_line: None,
                            content,
                            raw_content,
                        });
                        old_line += 1;
                    }
                    Some(b'\\') => parsed_lines.push(DiffLine {
                        kind: DiffLineKind::NoNewline,
                        old_line: None,
                        new_line: None,
                        content: String::from_utf8_lossy(trim_line_ending(line)).into_owned(),
                        raw_content: trim_line_ending(line).to_vec(),
                    }),
                    _ => return Err(DiffParseError::new("invalid unified diff line")),
                }
                index += 1;
            }
            hunks.push(DiffHunk {
                index: hunks.len(),
                old_start,
                old_count,
                new_start,
                new_count,
                header: hunk_header,
                raw: lines[hunk_start..index].concat(),
                lines: parsed_lines,
            });
        }

        document.files.push(DiffFile {
            old_path,
            new_path,
            header,
            hunks,
            binary,
        });
    }

    Ok(document)
}

fn parse_paths(header: &[u8]) -> Result<(String, String), DiffParseError> {
    let text = String::from_utf8_lossy(header);
    let old = text.lines().find_map(|line| line.strip_prefix("--- "));
    let new = text.lines().find_map(|line| line.strip_prefix("+++ "));
    if let (Some(old), Some(new)) = (old, new) {
        return Ok((normalize_diff_path(old), normalize_diff_path(new)));
    }
    let mut paths = text
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("diff --git "))
        .ok_or_else(|| DiffParseError::new("diff is missing file paths"))?
        .split_whitespace();
    let old = paths
        .next()
        .ok_or_else(|| DiffParseError::new("diff is missing old path"))?;
    let new = paths
        .next()
        .ok_or_else(|| DiffParseError::new("diff is missing new path"))?;
    Ok((normalize_diff_path(old), normalize_diff_path(new)))
}

fn normalize_diff_path(path: &str) -> String {
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
        .to_owned()
}

fn parse_hunk_header(line: &[u8]) -> Result<(u64, u64, u64, u64, String), DiffParseError> {
    let header = String::from_utf8_lossy(trim_line_ending(line)).into_owned();
    let range_end = header[3..]
        .find(" @@")
        .map(|offset| offset + 3)
        .ok_or_else(|| DiffParseError::new("invalid hunk header"))?;
    let mut ranges = header[3..range_end].split_whitespace();
    let (old_start, old_count) = parse_range(
        ranges
            .next()
            .and_then(|value| value.strip_prefix('-'))
            .ok_or_else(|| DiffParseError::new("invalid old hunk range"))?,
    )?;
    let (new_start, new_count) = parse_range(
        ranges
            .next()
            .and_then(|value| value.strip_prefix('+'))
            .ok_or_else(|| DiffParseError::new("invalid new hunk range"))?,
    )?;
    Ok((old_start, old_count, new_start, new_count, header))
}

fn parse_range(value: &str) -> Result<(u64, u64), DiffParseError> {
    let mut parts = value.split(',');
    let start = parts
        .next()
        .and_then(|part| part.parse().ok())
        .ok_or_else(|| DiffParseError::new("invalid hunk range start"))?;
    let count = parts
        .next()
        .map(str::parse)
        .transpose()
        .map_err(|_| DiffParseError::new("invalid hunk range count"))?
        .unwrap_or(1);
    Ok((start, count))
}

fn split_inclusive_lines(output: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, byte) in output.iter().enumerate() {
        if *byte == b'\n' {
            lines.push(&output[start..=index]);
            start = index + 1;
        }
    }
    if start < output.len() {
        lines.push(&output[start..]);
    }
    lines
}

fn trim_line_ending(line: &[u8]) -> &[u8] {
    let line = trim_line_feed(line);
    line.strip_suffix(b"\r").unwrap_or(line)
}

fn trim_line_feed(line: &[u8]) -> &[u8] {
    line.strip_suffix(b"\n").unwrap_or(line)
}

#[cfg(test)]
mod tests {
    use super::{DiffLineKind, parse_unified_diff};

    #[test]
    fn parses_multiple_hunks_and_line_numbers() {
        let diff = b"diff --git a/file.txt b/file.txt\nindex 111..222 100644\n--- a/file.txt\n+++ b/file.txt\n@@ -1,2 +1,2 @@\n one\n-two\n+changed\n@@ -5 +5,2 @@ tail\n five\n+six\n";
        let document = parse_unified_diff(diff).expect("valid diff");
        let file = &document.files[0];

        assert_eq!(file.old_path, "file.txt");
        assert_eq!(file.hunks.len(), 2);
        assert_eq!(file.hunks[0].lines[1].kind, DiffLineKind::Deletion);
        assert_eq!(file.hunks[0].lines[1].old_line, Some(2));
        assert_eq!(file.hunks[0].lines[2].new_line, Some(2));
        assert_eq!(file.hunks[1].new_count, 2);
    }

    #[test]
    fn accepts_an_empty_diff() {
        assert!(
            parse_unified_diff(b"")
                .expect("empty diff")
                .files
                .is_empty()
        );
    }

    #[test]
    fn preserves_crlf_bytes_for_patch_generation() {
        let diff = b"diff --git a/file.h b/file.h\nindex 111..222 100644\n--- a/file.h\n+++ b/file.h\n@@ -1 +1 @@\n-old\r\n+new\r\n";
        let document = parse_unified_diff(diff).expect("valid CRLF diff");
        let lines = &document.files[0].hunks[0].lines;

        assert_eq!(lines[0].content, "old");
        assert_eq!(lines[0].raw_content, b"old\r");
        assert_eq!(lines[1].content, "new");
        assert_eq!(lines[1].raw_content, b"new\r");
    }
}
