use anyhow::{Context, Result};
use std::path::PathBuf;

pub struct Cleaner {
    pub dir: Option<PathBuf>,
}

impl Drop for Cleaner {
    fn drop(&mut self) {
        if let Some(path) = &self.dir {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

pub fn store() -> Result<PathBuf> {
    let base = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .context("failed to locate data directory")?;
    let path = base.join("deal").join("grammars");
    std::fs::create_dir_all(&path).context("failed to create grammar directory")?;
    Ok(path)
}

pub fn add(langs: &[String]) -> Result<()> {
    let target = store()?;
    unsafe {
        std::env::set_var("TREE_SITTER_LANGUAGE_PACK_CACHE", &target);
        std::env::set_var("TSLP_CACHE_DIR", &target);
    }
    for lang in langs {
        let tag = lang.trim().to_lowercase();
        match tree_sitter_language_pack::get_language(&tag) {
            Ok(_) => println!(
                "  \x1b[38;2;166;227;161m✓\x1b[0m \x1b[38;2;205;214;244madded grammar for '{tag}'\x1b[0m"
            ),
            Err(why) => eprintln!(
                "  \x1b[38;2;243;139;168m✗\x1b[0m \x1b[38;2;205;214;244mfailed to add '{tag}': {why}\x1b[0m"
            ),
        }
    }
    Ok(())
}

pub fn remove(langs: &[String]) -> Result<()> {
    let target = store()?;
    for lang in langs {
        let tag = lang.trim().to_lowercase();
        let mut hit = false;
        if let Ok(entries) = std::fs::read_dir(&target) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if name.contains(&tag) {
                    if std::fs::remove_file(entry.path()).is_ok()
                        || std::fs::remove_dir_all(entry.path()).is_ok()
                    {
                        hit = true;
                    }
                }
            }
        }
        if hit {
            println!(
                "  \x1b[38;2;166;227;161m✓\x1b[0m \x1b[38;2;205;214;244mremoved grammar for '{tag}'\x1b[0m"
            );
        } else {
            println!(
                "  \x1b[38;2;147;153;178m!\x1b[0m \x1b[38;2;205;214;244mno installed grammar found for '{tag}'\x1b[0m"
            );
        }
    }
    Ok(())
}