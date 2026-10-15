//! Writes to, reads from, or frees a named pasteboard, so a check can play the part of an app that copies
//! while the real program watches that same board (`FIGGHOST_PASTEBOARD`). It never touches the real
//! clipboard: a name is required.
//! `named_board <name> write <text> | read | release`

use macos_sys::pasteboard::{Content, Pasteboard, Write};
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(name), Some(verb)) = (args.first(), args.get(1)) else {
        eprintln!("usage: named_board <name> write <text> | read | release");
        exit(2);
    };
    if name.is_empty() || name == "general" {
        eprintln!("named_board: refusing to use the general pasteboard");
        exit(2);
    }
    let board = Pasteboard::named(name);
    match verb.as_str() {
        "write" => {
            let text = args.get(2).map_or("", String::as_str);
            board.write(Write::Text(text));
        }
        "read" => match board.read() {
            Some(Content::Text(text)) => println!("{text}"),
            _ => exit(1),
        },
        "release" => board.release(),
        other => {
            eprintln!("named_board: unknown verb {other}");
            exit(2);
        }
    }
}
