use std::path::Path;

pub fn display_path(path: &Path) -> String {
    let path = path.to_string_lossy();

    #[cfg(windows)]
    {
        let path = if let Some(network_path) = path.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{network_path}")
        } else if let Some(local_path) = path.strip_prefix(r"\\?\") {
            local_path.to_owned()
        } else {
            path.into_owned()
        };
        path.replace('/', r"\")
    }

    #[cfg(not(windows))]
    {
        path.into_owned()
    }
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::display_path;
    #[cfg(windows)]
    use std::path::Path;

    #[cfg(windows)]
    #[test]
    fn windows_paths_use_the_regular_display_form() {
        assert_eq!(
            display_path(Path::new(r"\\?\C:\Projects\repo/src/main.rs")),
            r"C:\Projects\repo\src\main.rs"
        );
        assert_eq!(
            display_path(Path::new(r"\\?\UNC\server\share/repo.txt")),
            r"\\server\share\repo.txt"
        );
    }
}
