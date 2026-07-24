//! Shared temporary repository fixtures.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

pub struct TestRepository {
    directory: TempDir,
}

impl TestRepository {
    pub fn init() -> Self {
        let directory = tempfile::tempdir().expect("create temporary repository");
        run_git(directory.path(), ["init", "-b", "main"]);
        run_git(directory.path(), ["config", "user.name", "GitAcorn Test"]);
        run_git(
            directory.path(),
            ["config", "user.email", "test@gitacorn.local"],
        );
        fs::write(directory.path().join("tracked.txt"), "initial\n").expect("write tracked file");
        run_git(directory.path(), ["add", "tracked.txt"]);
        run_git(directory.path(), ["commit", "-m", "initial"]);
        Self { directory }
    }

    pub fn path(&self) -> &Path {
        self.directory.path()
    }

    pub fn write(&self, path: &str, contents: &str) {
        fs::write(self.path().join(path), contents).expect("write fixture file");
    }

    pub fn git<I, S>(&self, args: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        run_git(self.path(), args);
    }
}

fn run_git<I, S>(directory: &Path, args: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new("git")
        .current_dir(directory)
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(args)
        .output()
        .expect("run fixture Git command");
    assert!(
        output.status.success(),
        "Git fixture command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(test)]
mod tests {
    use super::TestRepository;

    #[test]
    fn initializes_a_real_repository() {
        let repository = TestRepository::init();
        assert!(repository.path().join(".git").is_dir());
    }
}
