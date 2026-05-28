use std::collections::HashMap;

pub(super) fn default_bindings() -> HashMap<String, String> {
    [
        ("Super+1", "workspace 1"),
        ("Super+2", "workspace 2"),
        ("Super+3", "workspace 3"),
        ("Super+4", "workspace 4"),
        ("Super+5", "workspace 5"),
        ("Super+6", "workspace 6"),
        ("Super+7", "workspace 7"),
        ("Super+8", "workspace 8"),
        ("Super+9", "workspace 9"),
        ("Super+Shift+1", "move-to-workspace 1"),
        ("Super+Shift+2", "move-to-workspace 2"),
        ("Super+Shift+3", "move-to-workspace 3"),
        ("Super+Shift+4", "move-to-workspace 4"),
        ("Super+Shift+5", "move-to-workspace 5"),
        ("Super+Shift+6", "move-to-workspace 6"),
        ("Super+Shift+7", "move-to-workspace 7"),
        ("Super+Shift+8", "move-to-workspace 8"),
        ("Super+Shift+9", "move-to-workspace 9"),
        ("Super+T", "toggle-tiling"),
        ("Super+H", "force-split horizontal"),
        ("Super+V", "force-split vertical"),
        ("Super+Left", "resize-tile horizontal -5%"),
        ("Super+Right", "resize-tile horizontal 5%"),
        ("Super+Up", "resize-tile vertical -5%"),
        ("Super+Down", "resize-tile vertical 5%"),
        ("Super+Space", "toggle-launcher"),
        ("Super+L", "lock-session"),
        ("Super+Q", "close-window"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}
