fn main() {
    let mut arguments = std::env::args_os();
    let _ = arguments.next();
    if arguments.next().as_deref() == Some(std::ffi::OsStr::new("--git-acorn-sequence-editor")) {
        let Some(plan) = arguments.next() else {
            std::process::exit(2);
        };
        let Some(todo) = arguments.next() else {
            std::process::exit(2);
        };
        if std::fs::copy(plan, todo).is_err() {
            std::process::exit(1);
        }
        return;
    }
    let _ = fix_path_env::fix();
    git_acorn_desktop_lib::run();
}
