use std::{env, path::PathBuf, process::Command};

/// Resolve bundled AppImage helpers first, then use PATH for source builds.
pub fn command(name: &str) -> Command {
    if let Some(appdir) = env::var_os("APPDIR") {
        let bundled = PathBuf::from(appdir).join("usr/bin").join(name);
        if bundled.is_file() {
            return Command::new(bundled);
        }
    }
    Command::new(name)
}
