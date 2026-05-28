use super::{Action, Keybind, Modifiers, SplitDir};

pub(super) fn parse_keybind(input: &str) -> Result<Keybind, String> {
    let parts: Vec<&str> = input.split('+').collect();
    if parts.is_empty() {
        return Err(format!("empty keybind: {:?}", input));
    }

    let key_name = parts.last().unwrap().trim();
    let mut modifiers = Modifiers::empty();

    for part in &parts[..parts.len() - 1] {
        match part.trim().to_lowercase().as_str() {
            "super" | "win" | "mod4" | "logo" => modifiers |= Modifiers::SUPER,
            "shift" => modifiers |= Modifiers::SHIFT,
            "ctrl" | "control" => modifiers |= Modifiers::CTRL,
            "alt" | "mod1" => modifiers |= Modifiers::ALT,
            other => return Err(format!("unknown modifier: {:?}", other)),
        }
    }

    let keysym = keysym_from_name(key_name)?;

    Ok(Keybind { modifiers, keysym })
}

fn keysym_from_name(name: &str) -> Result<u32, String> {
    let lower = name.to_lowercase();

    let sym = match lower.as_str() {
        "0" => 0x30,
        "1" => 0x31,
        "2" => 0x32,
        "3" => 0x33,
        "4" => 0x34,
        "5" => 0x35,
        "6" => 0x36,
        "7" => 0x37,
        "8" => 0x38,
        "9" => 0x39,
        "a" => 0x61,
        "b" => 0x62,
        "c" => 0x63,
        "d" => 0x64,
        "e" => 0x65,
        "f" => 0x66,
        "g" => 0x67,
        "h" => 0x68,
        "i" => 0x69,
        "j" => 0x6a,
        "k" => 0x6b,
        "l" => 0x6c,
        "m" => 0x6d,
        "n" => 0x6e,
        "o" => 0x6f,
        "p" => 0x70,
        "q" => 0x71,
        "r" => 0x72,
        "s" => 0x73,
        "t" => 0x74,
        "u" => 0x75,
        "v" => 0x76,
        "w" => 0x77,
        "x" => 0x78,
        "y" => 0x79,
        "z" => 0x7a,
        "escape" | "esc" => 0xff1b,
        "return" | "enter" => 0xff0d,
        "tab" => 0xff09,
        "space" => 0x20,
        "backspace" => 0xff08,
        "delete" => 0xffff,
        "home" => 0xff50,
        "end" => 0xff57,
        "page_up" | "pageup" | "pgup" => 0xff55,
        "page_down" | "pagedown" | "pgdn" => 0xff56,
        "left" => 0xff51,
        "up" => 0xff52,
        "right" => 0xff53,
        "down" => 0xff54,
        "f1" => 0xffbe,
        "f2" => 0xffbf,
        "f3" => 0xffc0,
        "f4" => 0xffc1,
        "f5" => 0xffc2,
        "f6" => 0xffc3,
        "f7" => 0xffc4,
        "f8" => 0xffc5,
        "f9" => 0xffc6,
        "f10" => 0xffc7,
        "f11" => 0xffc8,
        "f12" => 0xffc9,
        "minus" | "-" => 0x2d,
        "equal" | "=" => 0x3d,
        "bracket_left" | "[" => 0x5b,
        "bracket_right" | "]" => 0x5d,
        "backslash" | "\\" => 0x5c,
        "semicolon" | ";" => 0x3b,
        "apostrophe" | "'" => 0x27,
        "grave" | "`" => 0x60,
        "comma" | "," => 0x2c,
        "period" | "." => 0x2e,
        "slash" | "/" => 0x2f,
        "print" | "printscreen" | "sysrq" => 0xff61,
        "scroll_lock" => 0xff14,
        "pause" => 0xff13,
        "insert" => 0xff63,
        "caps_lock" => 0xffe5,
        "num_lock" => 0xff7f,
        _ => {
            let val =
                xkbcommon::xkb::keysym_from_name(name, xkbcommon::xkb::KEYSYM_CASE_INSENSITIVE);
            if val == xkbcommon::xkb::Keysym::NoSymbol {
                return Err(format!("unknown key name: {:?}", name));
            }
            u32::from(val)
        }
    };

    Ok(sym)
}

pub(super) fn parse_action(input: &str) -> Result<Action, String> {
    let input = input.trim();
    let (cmd, rest) = input.split_once(char::is_whitespace).unwrap_or((input, ""));

    match cmd {
        "workspace" | "switch-workspace" => {
            let n = parse_workspace_number(rest)?;
            Ok(Action::SwitchWorkspace(n - 1))
        }
        "move-to-workspace" => {
            let n = parse_workspace_number(rest)?;
            Ok(Action::MoveToWorkspace(n - 1))
        }
        "toggle-tiling" => Ok(Action::ToggleTiling),
        "force-split" => match rest.trim() {
            "horizontal" | "h" => Ok(Action::ForceSplit(SplitDir::Horizontal)),
            "vertical" | "v" => Ok(Action::ForceSplit(SplitDir::Vertical)),
            other => Err(format!("unknown split direction: {:?}", other)),
        },
        "resize-tile" => {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() != 2 {
                return Err(format!(
                    "resize-tile expects <dir> <delta>, got: {:?}",
                    rest
                ));
            }
            let dir = match parts[0] {
                "horizontal" | "h" => SplitDir::Horizontal,
                "vertical" | "v" => SplitDir::Vertical,
                other => return Err(format!("unknown direction: {:?}", other)),
            };
            let delta_str = parts[1].trim_end_matches('%');
            let delta: f32 = delta_str
                .parse()
                .map_err(|_| format!("invalid delta: {:?}", parts[1]))?;
            Ok(Action::ResizeTile {
                dir,
                delta: delta / 100.0,
            })
        }
        "close" | "close-window" => Ok(Action::CloseWindow),
        "toggle-launcher" => Ok(Action::ToggleLauncher),
        "lock" | "lock-session" => Ok(Action::LockSession),
        "reload-config" => Ok(Action::ReloadConfig),
        "quit" | "exit" => Ok(Action::Quit),
        other => Err(format!("unknown action: {:?}", other)),
    }
}

fn parse_workspace_number(input: &str) -> Result<usize, String> {
    let n: usize = input
        .trim()
        .parse()
        .map_err(|_| format!("invalid workspace number: {:?}", input))?;
    if !(1..=9).contains(&n) {
        return Err(format!("workspace number must be 1-9, got {}", n));
    }
    Ok(n)
}
