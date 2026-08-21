use super::*;

#[test]
fn parses_and_normalizes_desktop_entry() {
    let app = DesktopApp::parse("[Desktop Entry]\nType=Application\nName=Firefox\nExec=firefox --new-window %u\nCategories=Network;WebBrowser;\nIcon=firefox.png\n").unwrap();
    assert_eq!(app.name, "Firefox");
    assert_eq!(app.program, "firefox");
    assert_eq!(app.args, ["--new-window"]);
    assert_eq!(app.categories, ["network", "webbrowser"]);
    assert_eq!(app.icon_name.as_deref(), Some("firefox"));
}

#[test]
fn rejects_hidden_non_application_and_missing_try_exec() {
    for (raw, expected) in [
        ("[Desktop Entry]\nType=Link\nName=X\nExec=x\n", "unsupported-type"),
        ("[Desktop Entry]\nType=Application\nName=X\nExec=x\nHidden=true\n", "hidden-or-nodisplay"),
        ("[Desktop Entry]\nType=Application\nName=X\nExec=x\nTryExec=/definitely/missing/meridian\n", "tryexec-unavailable"),
    ] {
        assert_eq!(DesktopApp::parse(raw).unwrap_err(), expected);
    }
}

#[test]
fn visibility_is_meridian_specific() {
    assert!(DesktopApp::parse(
        "[Desktop Entry]\nType=Application\nName=X\nExec=x\nOnlyShowIn=Meridian;\n"
    )
    .is_ok());
    assert_eq!(
        DesktopApp::parse("[Desktop Entry]\nType=Application\nName=X\nExec=x\nOnlyShowIn=GNOME;\n")
            .unwrap_err(),
        "onlyshowin-excludes-meridian"
    );
    assert_eq!(
        DesktopApp::parse(
            "[Desktop Entry]\nType=Application\nName=X\nExec=x\nNotShowIn=Meridian;\n"
        )
        .unwrap_err(),
        "notshowin-includes-meridian"
    );
}

#[test]
fn exec_tokenization_removes_field_codes_and_preserves_quotes() {
    assert_eq!(
        parse_exec_argv("app \"two words\" escaped\\ space %u %%"),
        ["app", "two words", "escaped space", "%"]
    );
}

#[test]
fn helper_and_broken_standalone_entries_are_not_launcher_apps() {
    for desktop_id in NON_LAUNCHER_DESKTOP_IDS {
        assert!(!is_primary_launcher_entry(desktop_id));
    }
    for desktop_id in [
        "foot.desktop",
        "thunar.desktop",
        "org.gnome.Calculator.desktop",
        "org.gnome.TextEditor.desktop",
    ] {
        assert!(is_primary_launcher_entry(desktop_id));
    }
}
