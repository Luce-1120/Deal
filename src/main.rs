mod app;
mod clean;
mod cli;
mod git;
mod model;
mod syntax;
mod ui;

use anyhow::{Context, Result};
use app::{drive, App};
use clean::{add, remove, store, Cleaner};
use cli::{logo, usage, Action, Cli};
use git::harvest;
use gix::bstr::ByteSlice;
use std::path::{Path, PathBuf};
use ui::headline;

fn main() {
    let cli = <Cli as clap::Parser>::parse();

    if cli.version {
        logo();
        return;
    }
    if cli.help {
        usage();
        return;
    }

    if let Some(action) = cli.action {
        let result = match action {
            Action::Add { langs } => add(&langs),
            Action::Remove { langs } => remove(&langs),
        };
        if let Err(trouble) = result {
            eprintln!("\n\x1b[1;38;2;243;139;168mdeal: {trouble}\x1b[0m");
            std::process::exit(1);
        }
        return;
    }

    if let Err(trouble) = run(cli.path.as_deref()) {
        eprintln!("\n\x1b[1;38;2;243;139;168mdeal: {trouble}\x1b[0m");
        for cause in trouble.chain().skip(1) {
            eprintln!("      \x1b[38;2;147;153;178mcaused by: {cause}\x1b[0m");
        }
        eprintln!();
        std::process::exit(1);
    }
}

fn run(target: Option<&Path>) -> Result<()> {
    let temp = tempfile::tempdir().ok();
    let cleaner = Cleaner {
        dir: temp.as_ref().map(|d| d.path().to_path_buf()),
    };
    if let Some(t) = &temp {
        if let Ok(saved) = store() {
            if let Ok(entries) = std::fs::read_dir(&saved) {
                for entry in entries.flatten() {
                    let dest = t.path().join(entry.file_name());
                    let _ = std::fs::copy(entry.path(), dest);
                }
            }
        }
        unsafe {
            std::env::set_var("TREE_SITTER_LANGUAGE_PACK_CACHE", t.path());
            std::env::set_var("TSLP_CACHE_DIR", t.path());
        }
    }

    let root = target.map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));

    let repo = gix::discover(&root).with_context(|| {
        format!(
            "no git repository found at or above '{}'",
            root.display()
        )
    })?;

    let work = repo.workdir().map(Path::to_path_buf).with_context(|| {
        format!(
            "repository at '{}' is bare",
            repo.git_dir().display()
        )
    })?;

    let batch = harvest(&repo, &work)?;

    if batch.docs.is_empty() {
        println!(
            "\n  \x1b[38;2;166;227;161m✓\x1b[0m \x1b[38;2;205;214;244mworking tree matches HEAD\x1b[0m"
        );
        for skip in &batch.skips {
            println!("    \x1b[38;2;147;153;178mskipped {skip}\x1b[0m");
        }
        println!();
        return Ok(());
    }

    let brand = work
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "repository".to_string());

    let twig = repo
        .head_name()
        .ok()
        .flatten()
        .map(|n| n.shorten().to_str_lossy().into_owned())
        .unwrap_or_else(|| "detached".to_string());

    let mut app = App::new(
        batch,
        brand,
        twig,
        matches!(repo.committer(), Some(Ok(_))),
        work,
    );

    let mut terminal = ratatui::try_init().context(
        "failed to initialize terminal",
    )?;

    let outcome = drive(&mut terminal, &mut app, &repo);
    let _ = ratatui::try_restore();
    drop(cleaner);
    outcome?;

    if let Some(report) = &app.done {
        println!();
        println!(
            "  \x1b[1;38;2;166;227;161m✓\x1b[0m \x1b[1m{} commit{} committed to {}\x1b[0m",
            report.commits.len(),
            if report.commits.len() == 1 { "" } else { "s" },
            app.twig
        );
        for (id, note) in &report.commits {
            println!(
                "    \x1b[38;2;137;180;250m{}\x1b[0m  \x1b[38;2;205;214;244m{}\x1b[0m",
                id.to_hex_with_len(7),
                headline(note)
            );
        }
        if let Some(gripe) = &report.gripe {
            println!("    \x1b[38;2;250;179;135m!\x1b[0m \x1b[38;2;205;214;244m{gripe}\x1b[0m");
        }
        println!();
    }

    Ok(())
}