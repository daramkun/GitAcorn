use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitSummary {
    pub oid: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub authored_at: i64,
    pub subject: String,
    pub body: String,
    pub references: Vec<String>,
    pub remote_only: bool,
    pub lane: usize,
    pub lane_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryPage {
    pub commits: Vec<CommitSummary>,
    pub next_cursor: Option<String>,
}

pub fn parse_history_records(input: &[u8]) -> Result<Vec<CommitSummary>, String> {
    let mut commits = Vec::new();
    for record in input.split(|byte| *byte == 0x1e) {
        let record = record.strip_prefix(b"\r\n").unwrap_or(record);
        let record = record.strip_prefix(b"\n").unwrap_or(record);
        let record = record.strip_suffix(b"\n").unwrap_or(record);
        if record.is_empty() {
            continue;
        }
        let fields: Vec<&[u8]> = record.split(|byte| *byte == 0).collect();
        if fields.len() < 8 {
            return Err(format!(
                "history record has {} fields; expected 8",
                fields.len()
            ));
        }
        let text = |field: &[u8]| String::from_utf8_lossy(field).into_owned();
        let authored_at = text(fields[4])
            .parse()
            .map_err(|_| "history timestamp is not an integer".to_owned())?;
        commits.push(CommitSummary {
            oid: text(fields[0]),
            parents: text(fields[1])
                .split_whitespace()
                .map(ToOwned::to_owned)
                .collect(),
            author_name: text(fields[2]),
            author_email: text(fields[3]),
            authored_at,
            subject: text(fields[5]),
            body: text(fields[6]).trim_end().to_owned(),
            references: text(fields[7])
                .split(", ")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            remote_only: false,
            lane: 0,
            lane_count: 1,
        });
    }
    assign_lanes(&mut commits);
    Ok(commits)
}

fn assign_lanes(commits: &mut [CommitSummary]) {
    let mut lanes: Vec<String> = Vec::new();
    for commit in commits {
        let lane = lanes
            .iter()
            .position(|oid| oid == &commit.oid)
            .unwrap_or_else(|| {
                lanes.insert(0, commit.oid.clone());
                0
            });
        commit.lane = lane;
        if let Some(first_parent) = commit.parents.first() {
            lanes[lane] = first_parent.clone();
            for (offset, parent) in commit.parents.iter().skip(1).enumerate() {
                if !lanes.contains(parent) {
                    lanes.insert(lane + offset + 1, parent.clone());
                }
            }
        } else {
            lanes.remove(lane);
        }
        lanes.dedup();
        commit.lane_count = lanes.len().max(commit.lane + 1).max(1);
    }
}

#[cfg(test)]
mod tests {
    use super::parse_history_records;

    #[test]
    fn parses_machine_history_and_assigns_merge_lanes() {
        let input = concat!(
            "ccc\0bbb aaa\0Ada\0ada@example.com\01700000000\0Merge topic\0body\0HEAD -> refs/heads/main\0\x1e",
            "bbb\0aaa\0Ada\0ada@example.com\01699999999\0Topic\0\0refs/heads/topic\0\x1e",
            "aaa\0\0Lin\0lin@example.com\01699999998\0Root\0\0\0\x1e"
        );
        let commits = parse_history_records(input.as_bytes()).expect("valid history");

        assert_eq!(commits.len(), 3);
        assert_eq!(commits[0].parents, ["bbb", "aaa"]);
        assert_eq!(commits[0].references, ["HEAD -> refs/heads/main"]);
        assert_eq!(commits[1].lane, 0);
        assert!(commits[0].lane_count >= 2);
    }
}
