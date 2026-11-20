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
    let commit = if allow_replace {
        std::fs::rename(&temp, target)
    } else {
        // Linking within the same directory atomically refuses an existing destination.
        std::fs::hard_link(&temp, target).map(|()| { let _ = std::fs::remove_file(&temp); })
    };
    commit.map_err(|e| {
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

/// Extract known metadata fields while keeping diagnostic chatter out of the summary.
pub fn information_sections(report: &str) -> Vec<(String, Vec<(String, String)>)> {
    let mut volume = Vec::new();
    let mut dates = Vec::new();
    let mut boot = Vec::new();
    for line in report.lines() {
        let Some((key, value)) = line.split_once(':') else { continue };
        let key = key.trim();
        let value = value.trim();
        let label = match key {
            "Volume Id" => "Volume label",
            "Volume Set Id" => "Volume set",
            "Publisher Id" => "Publisher",
            "Preparer Id" => "Prepared by",
            "App Id" => "Application",
            "System Id" => "System",
            "CopyrightFile" => "Copyright file",
            "Abstract File" => "Abstract file",
            "Biblio File" => "Bibliography file",
            "Creation Time" | "Modif. Time" | "Expir. Time" | "Eff. Time" => {
                let label = match key {
                    "Creation Time" => "Created", "Modif. Time" => "Modified",
                    "Expir. Time" => "Expires", _ => "Effective from",
                };
                dates.push((label.into(), format_iso_date(value)));
                continue;
            }
            _ if key.starts_with("El Torito") || key.starts_with("System area") || key.starts_with("System Area") || key.starts_with("MBR") || key.starts_with("GPT") || key.starts_with("APM") => {
                boot.push((key.into(), value.into()));
                continue;
            }
            _ => continue,
        };
        volume.push((label.into(), if value.is_empty() { "Not specified".into() } else { value.into() }));
    }
    if boot.is_empty() {
        boot.push(("Boot metadata".into(), if report.contains("No El Torito information") && report.contains("No System Area") {
            "No El Torito or system-area boot metadata detected"
        } else { "See the full report for available boot metadata" }.into()));
    }
    vec![("Volume".into(), volume), ("Dates".into(), dates), ("Boot information".into(), boot)]
}

fn format_iso_date(value: &str) -> String {
    if value.is_empty() || value.bytes().all(|byte| byte == b'0') { return "Not specified".into(); }
    if value.len() >= 14 && value.bytes().all(|byte| byte.is_ascii_digit()) {
        return format!("{}-{}-{} {}:{}:{}", &value[..4], &value[4..6], &value[6..8], &value[8..10], &value[10..12], &value[12..14]);
    }
    value.into()
}

#[cfg(test)]
mod information_tests {
    use super::*;
    #[test]
    fn metadata_is_grouped_and_dates_are_readable() {
        let sections = information_sections("xorriso : NOTE : chatter\nVolume Id : EXAMPLE\nPublisher Id : \nCreation Time: 2026112015302400\nExpir. Time: 0000000000000000\nEl Torito boot img : 1 BIOS\nGPT disk GUID : abc");
        assert_eq!(sections[0].1, vec![("Volume label".into(), "EXAMPLE".into()), ("Publisher".into(), "Not specified".into())]);
        assert_eq!(sections[1].1[0].1, "2026-11-20 15:30:24");
        assert_eq!(sections[1].1[1].1, "Not specified");
        assert_eq!(sections[2].1.len(), 2);
    }
    #[test]
    fn missing_boot_data_is_explicit() {
        let sections = information_sections("xorriso : NOTE : No El Torito information was loaded\nxorriso : NOTE : No System Area was loaded");
        assert!(sections[2].1[0].1.starts_with("No El Torito"));
        assert_eq!(format_iso_date("unknown"), "unknown");
    }
}

#[cfg(test)]
mod save_tests {
    use super::*;
    #[test]
    fn saves_a_copy_and_preserves_source_and_existing_target() {
        let root = std::env::temp_dir().join(format!("isoweaver-save-test-{}", std::process::id()));
        std::fs::create_dir_all(root.join("files")).unwrap();
        std::fs::write(root.join("files/original.txt"), "original").unwrap();
        let input = root.join("source.iso");
        let result = std::process::Command::new("xorriso").args(["-as", "mkisofs", "-o"])
            .arg(&input).arg(root.join("files")).output().unwrap();
        assert!(result.status.success());
        let original = std::fs::read(&input).unwrap();
        let target = root.join("source_edited.iso");
        let changes = vec![Change::CreateDirectory { image_path: "/new-folder".into() }];
        save_image(&input, &target, &changes, false).unwrap();
        assert_eq!(std::fs::read(&input).unwrap(), original);
        let saved = std::fs::read(&target).unwrap();
        assert!(save_image(&input, &target, &changes, false).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), saved);
        let listing = std::process::Command::new("xorriso").arg("-indev").arg(&target)
            .args(["-ls", "/", "--", "-end"]).output().unwrap();
        assert!(listing.status.success());
        assert!(String::from_utf8_lossy(&listing.stdout).contains("new-folder"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
