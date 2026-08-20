#![deny(unsafe_code)]
//! Minimal WebKitGTK/Wayland runtime proof for Meridian-owned UI.
//!
//! This is intentionally a standalone diagnostic process, not a replacement
//! for `meridian-shell` yet. It loads only compiled-in assets, uses ephemeral
//! WebKit storage and exposes no bridge capability.

mod bridge;
mod document;
mod gtk_host;
mod icon_service;
mod ipc;

use document::ThemeChoice;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SurfaceChoice {
    Launcher,
    Panel,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("meridian-ui-runtime: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let Some((theme, surface)) = parse_args(std::env::args().skip(1))? else {
        print_help();
        return Ok(());
    };
    require_wayland_backend()?;

    gtk_host::run(theme, surface);
    Ok(())
}

fn parse_args(
    args: impl Iterator<Item = String>,
) -> Result<Option<(ThemeChoice, SurfaceChoice)>, String> {
    let mut theme = ThemeChoice::Dark;
    let mut surface = SurfaceChoice::Launcher;
    for argument in args {
        if argument == "--help" || argument == "-h" {
            return Ok(None);
        }
        if let Some(value) = argument.strip_prefix("--theme=") {
            theme = ThemeChoice::parse(value)?;
            continue;
        }
        if let Some(value) = argument.strip_prefix("--surface=") {
            surface = match value {
                "launcher" => SurfaceChoice::Launcher,
                "panel" => SurfaceChoice::Panel,
                _ => {
                    return Err(format!(
                        "unsupported surface {value:?}; expected launcher or panel"
                    ))
                }
            };
            continue;
        }
        return Err(format!("unknown argument {argument:?}; try --help"));
    }
    Ok(Some((theme, surface)))
}

fn print_help() {
    println!("Usage: meridian-ui-runtime [--theme=dark|light] [--surface=launcher|panel]");
    println!("Opens a local Meridian WebKitGTK shell surface.");
}

fn require_wayland_backend() -> Result<(), String> {
    if let Some(value) = std::env::var_os("GDK_BACKEND") {
        if value != "wayland" {
            return Err(format!(
                "GDK_BACKEND must be wayland for this proof, got {value:?}"
            ));
        }
    } else {
        std::env::set_var("GDK_BACKEND", "wayland");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_small_and_explicit() {
        assert_eq!(
            parse_args(["--theme=light".to_string()].into_iter()),
            Ok(Some((ThemeChoice::Light, SurfaceChoice::Launcher)))
        );
        assert_eq!(
            parse_args(["--surface=panel".to_string()].into_iter()),
            Ok(Some((ThemeChoice::Dark, SurfaceChoice::Panel)))
        );
        assert_eq!(parse_args(["--help".to_string()].into_iter()), Ok(None));
        assert!(parse_args(["--inspect".to_string()].into_iter()).is_err());
    }
}
