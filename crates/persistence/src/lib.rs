//! Local settings and SQLite-backed session persistence will be implemented here.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistenceSettings {
    pub schema_version: u16,
}

impl Default for PersistenceSettings {
    fn default() -> Self {
        Self { schema_version: 1 }
    }
}

#[cfg(test)]
mod tests {
    use super::PersistenceSettings;

    #[test]
    fn settings_start_at_schema_version_one() {
        assert_eq!(PersistenceSettings::default().schema_version, 1);
    }
}
