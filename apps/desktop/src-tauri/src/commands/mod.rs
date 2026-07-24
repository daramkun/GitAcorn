use crate::dto::AppInfoDto;

#[tauri::command]
pub fn app_info() -> AppInfoDto {
    AppInfoDto::current()
}
