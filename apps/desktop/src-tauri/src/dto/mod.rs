use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoDto {
    pub schema_version: u16,
    pub name: &'static str,
    pub version: &'static str,
    pub runtime: &'static str,
}

impl AppInfoDto {
    pub const fn current() -> Self {
        Self {
            schema_version: 1,
            name: "GitAcorn",
            version: env!("CARGO_PKG_VERSION"),
            runtime: "Tauri 2",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppInfoDto;

    #[test]
    fn app_info_contract_uses_versioned_camel_case_fields() {
        let value = serde_json::to_value(AppInfoDto::current()).expect("serializable DTO");

        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["name"], "GitAcorn");
        assert_eq!(value["runtime"], "Tauri 2");
        assert!(value.get("schema_version").is_none());
    }
}
