use clap::{CommandFactory, Parser, Subcommand};
use figlet_rs::FIGlet;
use std::path::PathBuf;

const LOGO: &[&str] = &[
    "\x1b[38;2;100;200;235m",
    "\x1b[38;2;90;170;245m",
    "\x1b[38;2;80;140;250m",
    "\x1b[38;2;95;120;250m",
    "\x1b[38;2;125;110;250m",
    "\x1b[38;2;155;110;245m",
];

const TINT: &[&str] = &[
    "\x1b[1;38;2;130;215;245m",
    "\x1b[1;38;2;122;190;250m",
    "\x1b[1;38;2;115;165;252m",
    "\x1b[1;38;2;130;150;252m",
    "\x1b[1;38;2;155;142;252m",
    "\x1b[1;38;2;180;142;250m",
];

#[derive(Parser, Debug)]
#[command(name = "deal", disable_help_flag = true, disable_version_flag = true)]
pub struct Cli {
    #[arg(short = 'v', long, help = "Show version")]
    pub version: bool,

    #[arg(short = 'h', long, help = "Show help")]
    pub help: bool,

    #[arg(
        short = 'p',
        long,
        value_name = "PATH",
        help = "Target repository path"
    )]
    pub path: Option<PathBuf>,

    #[command(subcommand)]
    pub action: Option<Action>,
}

#[derive(Subcommand, Debug)]
pub enum Action {
    #[command(name = "add", about = "Add grammars")]
    Add {
        #[arg(required = true, value_name = "LANG", help = "Grammars to add")]
        langs: Vec<String>,
    },
    #[command(name = "remove", about = "Remove grammars")]
    Remove {
        #[arg(required = true, value_name = "LANG", help = "Grammars to remove")]
        langs: Vec<String>,
    },
}

pub fn logo() {
    let stamp = env!("CARGO_PKG_VERSION");
    let Ok(font) = FIGlet::slant() else {
        println!("DEAL v{stamp}");
        return;
    };
    let Some(word) = font.convert("DEAL") else {
        println!("DEAL v{stamp}");
        return;
    };
    let Some(tag) = font.convert(&format!("v{stamp}")) else {
        println!("DEAL v{stamp}");
        return;
    };

    println!();
    let left = word.to_string();
    let right = tag.to_string();
    for (i, (a, b)) in left.lines().zip(right.lines()).enumerate() {
        let lhue = LOGO[i % LOGO.len()];
        let thue = TINT[i % TINT.len()];
        println!("{lhue}{:<28}\x1b[0m {thue}{}\x1b[0m", a, b);
    }
    println!();
}

pub fn usage() {
    println!(
        "\n\x1b[1;38;2;137;180;250mUSAGE:\x1b[0m\n  \x1b[38;2;205;214;244mdeal [OPTIONS]\x1b[0m\n  \x1b[38;2;205;214;244mdeal add <LANG...>\x1b[0m\n  \x1b[38;2;205;214;244mdeal remove <LANG...>\x1b[0m\n\n\x1b[1;38;2;137;180;250mOPTIONS:\x1b[0m"
    );

    for arg in Cli::command().get_arguments() {
        let short = arg.get_short().map_or(String::new(), |c| format!("-{c}, "));
        let long = arg.get_long().map_or(String::new(), |l| format!("--{l}"));
        let slot = arg
            .get_value_names()
            .map_or(String::new(), |v| format!(" <{}>", v.join(" ")));
        let text = arg.get_help().map_or(String::new(), |h| h.to_string());

        println!(
            "  \x1b[38;2;180;190;254m{:<24}\x1b[0m \x1b[38;2;147;153;178m{text}\x1b[0m",
            format!("{short}{long}{slot}")
        );
    }

    println!(
        "\n\x1b[1;38;2;137;180;250mCOMMANDS:\x1b[0m\n  \x1b[38;2;180;190;254m{:<24}\x1b[0m \x1b[38;2;147;153;178mAdd grammars\x1b[0m\n  \x1b[38;2;180;190;254m{:<24}\x1b[0m \x1b[38;2;147;153;178mRemove grammars\x1b[0m",
        "add <LANG...>", "remove <LANG...>"
    );

    println!(
        "\n\x1b[1;38;2;137;180;250mKEYS:\x1b[0m\n  \x1b[38;2;205;214;244mj k\x1b[0m move   \x1b[38;2;205;214;244mh l\x1b[0m fold   \x1b[38;2;205;214;244m0-9\x1b[0m deal   \x1b[38;2;205;214;244mm\x1b[0m message   \x1b[38;2;205;214;244mc\x1b[0m commit   \x1b[38;2;205;214;244m?\x1b[0m help\x1b[0m\n"
    );
}