use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use super::{
    is_executable_available, normalize_icon_name, parse_categories, parse_exec_argv, DesktopApp,
    LauncherState,
};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        path.push(format!(
            "meridian-shell-launcher-{label}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, "#!/bin/sh\nexit 0\n").expect("write executable");
    let mut perms = fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(perms.mode() | 0o755);
    fs::set_permissions(path, perms).expect("chmod");
}

#[test]
fn parses_valid_desktop_entry() {
    let raw = "\
[Desktop Entry]
Type=Application
Name=Firefox
Exec=firefox --new-window %u
Terminal=false
Categories=Network;WebBrowser;
Icon=firefox.png
";

    let app = DesktopApp::from_desktop_entry_str_with_reason(raw).expect("valid app");
    assert_eq!(app.name, "Firefox");
    assert_eq!(app.program, "firefox");
    assert_eq!(app.args, vec!["--new-window"]);
    assert!(!app.terminal);
    assert_eq!(app.categories, vec!["network", "webbrowser"]);
    assert_eq!(app.icon_name.as_deref(), Some("firefox"));
}

#[test]
fn rejects_hidden_nodisplay_and_non_application_entries() {
    let unsupported = DesktopApp::from_desktop_entry_str_with_reason(
        "[Desktop Entry]\nType=Link\nName=X\nExec=x\n",
    );
    assert_eq!(unsupported.unwrap_err(), "unsupported-type");

    let hidden = DesktopApp::from_desktop_entry_str_with_reason(
        "[Desktop Entry]\nType=Application\nName=X\nExec=x\nHidden=true\n",
    );
    assert_eq!(hidden.unwrap_err(), "hidden-or-nodisplay");

    let no_display = DesktopApp::from_desktop_entry_str_with_reason(
        "[Desktop Entry]\nType=Application\nName=X\nExec=x\nNoDisplay=true\n",
    );
    assert_eq!(no_display.unwrap_err(), "hidden-or-nodisplay");
}

#[test]
fn desktop_visibility_respects_meridian_environment_keys() {
    assert!(DesktopApp::from_desktop_entry_str_with_reason(
        "[Desktop Entry]\nType=Application\nName=X\nExec=x\nOnlyShowIn=Meridian;\n"
    )
    .is_ok());
    let other_desktop = DesktopApp::from_desktop_entry_str_with_reason(
        "[Desktop Entry]\nType=Application\nName=X\nExec=x\nOnlyShowIn=GNOME;\n",
    );
    assert_eq!(other_desktop.unwrap_err(), "onlyshowin-excludes-meridian");

    let hidden_from_meridian = DesktopApp::from_desktop_entry_str_with_reason(
        "[Desktop Entry]\nType=Application\nName=X\nExec=x\nNotShowIn=Meridian;\n",
    );
    assert_eq!(
        hidden_from_meridian.unwrap_err(),
        "notshowin-includes-meridian"
    );
}

#[test]
fn exec_field_codes_are_removed() {
    assert_eq!(
        parse_exec_argv("firefox --new-window %u %% --name=%c"),
        vec!["firefox", "--new-window", "%", "--name="]
    );
}

#[test]
fn exec_quotes_are_handled() {
    assert_eq!(
        parse_exec_argv("app \"two words\" 'three words' escaped\\ space"),
        vec!["app", "two words", "three words", "escaped space"]
    );
}

#[test]
fn parses_categories_and_normalizes_icon_names() {
    assert_eq!(
        parse_categories("Network; WebBrowser;;"),
        vec!["network", "webbrowser"]
    );
    assert_eq!(
        normalize_icon_name("firefox.svg").as_deref(),
        Some("firefox")
    );
    assert_eq!(
        normalize_icon_name("/opt/icons/firefox.png").as_deref(),
        Some("/opt/icons/firefox.png")
    );
    assert_eq!(normalize_icon_name("   "), None);
}

#[test]
fn try_exec_rejects_missing_binary() {
    assert!(!is_executable_available(""));
    assert!(!is_executable_available(
        "/definitely/missing/meridian-test-binary"
    ));
    let app = DesktopApp::from_desktop_entry_str_with_reason(
        "[Desktop Entry]\nType=Application\nName=X\nExec=x\nTryExec=/definitely/missing/meridian-test-binary\n",
    );
    assert_eq!(app.unwrap_err(), "tryexec-unavailable");
}

#[cfg(unix)]
#[test]
fn load_from_dirs_deduplicates_sorts_and_checks_try_exec() {
    let temp = TempDir::new("load");
    let apps_dir = temp.path().join("applications");
    fs::create_dir_all(&apps_dir).expect("apps dir");
    let helper = temp.path().join("helper");
    make_executable(&helper);

    fs::write(
        apps_dir.join("b.desktop"),
        format!(
            "[Desktop Entry]\nType=Application\nName=Beta\nExec={}\nTryExec={}\n",
            helper.display(),
            helper.display()
        ),
    )
    .expect("write beta");
    fs::write(
        apps_dir.join("a.desktop"),
        format!(
            "[Desktop Entry]\nType=Application\nName=Alpha\nExec={}\n",
            helper.display()
        ),
    )
    .expect("write alpha");
    fs::write(
        apps_dir.join("duplicate.desktop"),
        format!(
            "[Desktop Entry]\nType=Application\nName=Alpha\nExec={}\n",
            helper.display()
        ),
    )
    .expect("write duplicate");
    fs::write(
        apps_dir.join("ignored.desktop"),
        "[Desktop Entry]\nType=Application\nName=Ignored\nExec=ignored\nTryExec=/missing\n",
    )
    .expect("write ignored");

    let apps = DesktopApp::load_from_dirs(vec![apps_dir]);
    assert_eq!(
        apps.iter().map(|app| app.name.as_str()).collect::<Vec<_>>(),
        vec!["Alpha", "Beta"]
    );
}

#[test]
fn launcher_state_tracks_open_close_and_apps() {
    let apps = vec![DesktopApp::new(
        "Firefox".to_string(),
        vec!["firefox".to_string()],
        false,
    )];
    let mut state = LauncherState::new_with_apps(apps);
    assert!(!state.open);
    assert_eq!(state.apps.len(), 1);
    state.close();
    assert!(!state.open);
}
