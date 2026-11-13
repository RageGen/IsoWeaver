use std::{path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

#[derive(Clone, Debug)]
pub enum Change {
    Add { source: PathBuf, image_path: String },
    Delete { image_path: String, directory: bool },
    CreateDirectory { image_path: String },
    Rename { old_path: String, new_path: String },
}

pub fn inspect_boot(path: &Path) -> Result<String, String> {
    let output = crate::runtime::command("xorriso")
        .arg("-indev").arg(path)
        .args(["-report_el_torito", "plain", "-report_system_area", "plain", "-pvd_info", "-end"])
        .output().map_err(|e| format!("Could not start xorriso: {e}"))?;
    let mut report = String::from_utf8_lossy(&output.stdout).into_owned();
    report.push_str(&String::from_utf8_lossy(&output.stderr));
    if !output.status.success() { return Err(format!("xorriso could not inspect the image: {report}")); }
    Ok(report)
}

pub fn save_image(input: &Path, target: &Path, changes: &[Change], allow_replace: bool) -> Result<(), String> {
    if changes.is_empty() { return Err("There are no pending image changes.".into()); }
    if !target.parent().is_some_and(Path::exists) { return Err("The destination directory does not exist.".into()); }
    if target.exists() && !allow_replace { return Err(format!("Refusing to overwrite {}", target.display())); }
    if let Ok(meta) = std::fs::symlink_metadata(target) {
        if meta.file_type().is_symlink() || !meta.is_file() { return Err("The save target must be a regular file, not a symlink or directory.".into()); }
    }
    let boot_report = inspect_boot(input)?;
    let had_el_torito = !boot_report.contains("No El Torito information");
    let had_system_area = !boot_report.contains("No System Area was loaded");
    let temp = temporary_sibling(target)?;
    let mut command = crate::runtime::command("xorriso");
    command.arg("-indev").arg(input).arg("-outdev").arg(&temp);
    if had_el_torito || had_system_area { command.args(["-boot_image", "any", "replay"]); }
    command.args(["-overwrite", "on"]);
    for change in changes {
        match change {
            Change::Add { source, image_path } => {
                validate_image_path(image_path)?;
                if !source.exists() { return Err(format!("Added source no longer exists: {}", source.display())); }
                command.arg("-map").arg(source).arg(image_path);
            }
            Change::Delete { image_path, directory } => {
                validate_image_path(image_path)?;
                command.arg(if *directory { "-rm_r" } else { "-rm" }).arg(image_path).arg("--");
            }
            Change::CreateDirectory { image_path } => {
                validate_image_path(image_path)?;
                command.arg("-mkdir").arg(image_path).arg("--");
            }
            Change::Rename { old_path, new_path } => {
                validate_image_path(old_path)?;
                validate_image_path(new_path)?;
                command.arg("-move").arg(old_path).arg(new_path);
            }
        }
    }
    command.args(["-commit", "-end"]);
    let result = command.output().map_err(|e| format!("Could not start xorriso: {e}"))?;
    if !result.status.success() {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("xorriso failed ({}):\n{}", result.status, String::from_utf8_lossy(&result.stderr)));
    }
    let verify = match crate::runtime::command("xorriso").arg("-indev").arg(&temp).args(["-pvd_info", "-end"]).output() {
        Ok(result)=>result,
        Err(e)=> { let _=std::fs::remove_file(&temp); return Err(format!("Could not verify temporary image: {e}")); }
    };
    if !verify.status.success() {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("The rebuilt image did not pass xorriso verification:\n{}", String::from_utf8_lossy(&verify.stderr)));
    }
    if had_el_torito || had_system_area {
        let rebuilt_boot = match inspect_boot(&temp) { Ok(report)=>report, Err(e)=> { let _=std::fs::remove_file(&temp); return Err(e); } };
        if had_el_torito && rebuilt_boot.contains("No El Torito information") {
            let _ = std::fs::remove_file(&temp);
            return Err("The source image had El Torito boot data, but the rebuilt image does not. The original was preserved.".into());
        }
        if rebuilt_boot.contains("No System Area was loaded") {
            let _ = std::fs::remove_file(&temp);
            return Err("The source image had system-area boot data, but the rebuilt image does not. The original was preserved.".into());
        }
    }
    std::fs::rename(&temp, target).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        format!("Could not commit verified image to {}: {e}", target.display())
    })?;
    Ok(())
}

fn validate_image_path(value: &str) -> Result<(), String> {
    if !value.starts_with('/') || value.starts_with("//") || value.contains('\0') || value.split('/').any(|part| part == "." || part == "..") {
        return Err(format!("Unsafe ISO path: {value}"));
    }
    Ok(())
}

fn temporary_sibling(target: &Path) -> Result<PathBuf, String> {
    let parent = target.parent().ok_or_else(|| "Destination has no parent directory.".to_owned())?;
    let name = target.file_name().and_then(|n| n.to_str()).unwrap_or("image.iso");
    for attempt in 0..100 {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
        let candidate = parent.join(format!(".{name}.isoweaver-{}-{stamp}-{attempt}.tmp", std::process::id()));
        if !candidate.exists() { return Ok(candidate); }
    }
    Err("Could not allocate a temporary output name.".into())
}
