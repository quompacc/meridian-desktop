use meridian_config::Color;

pub const ACCENT_FOREGROUND: Color = Color::rgb(0x1a, 0x1b, 0x26);

#[allow(dead_code)]
pub mod spacing {
    pub const XS: i32 = 4;
    pub const SM: i32 = 6;
    pub const MD: i32 = 8;
    pub const LG: i32 = 10;
    pub const XL: i32 = 12;
    pub const XXL: i32 = 16;
}

#[allow(dead_code)]
pub mod radius {
    pub const SM: i32 = 0;
    pub const MD: i32 = 0;
    pub const LG: i32 = 0;
}

pub mod badge {
    pub const SIZE: i32 = 18;
    pub const CONTENT_GAP: i32 = 8;
    pub const RADIUS: i32 = super::radius::SM;
}

#[allow(dead_code)]
pub mod panel {
    pub const WORKSPACE_BUTTON_W: i32 = 28;
    pub const WORKSPACE_BUTTON_H: i32 = 28;
    pub const WORKSPACE_BUTTON_Y: i32 = 7;
    pub const WORKSPACE_BUTTON_GAP: i32 = 4;
    pub const LEFT_PADDING: i32 = 8;
    pub const LAUNCHER_BUTTON_W: i32 = 58;
    pub const PINNED_TILE_W: i32 = 44;
    pub const PINNED_TILE_GAP: i32 = 4;
    pub const PINNED_SECTION_GAP: i32 = 12;
    pub const TRAY_SLOT_W: i32 = 60;
    pub const WORKSPACE_IND_W: i32 = 56;
    pub const CLOCK_PADDING_H: i32 = 8;
    pub const RIGHT_PADDING: i32 = 10;
    pub const OUTER_RADIUS: i32 = super::radius::LG;
    pub const GROUP_RADIUS: i32 = super::radius::MD;
    pub const BUTTON_RADIUS: i32 = super::radius::SM;
    pub const CLOCK_RADIUS: i32 = super::radius::MD;
}

#[allow(dead_code)]
pub mod launcher {
    pub const APP_ROW_H: i32 = 38;
    pub const SEARCH_H: i32 = 44;
    pub const HEADER_H: i32 = 22;
    pub const OUTER_PADDING: i32 = 16;
    pub const INNER_PADDING: i32 = 12;
    pub const ROW_GAP: i32 = 4;
    pub const LIST_TOP_GAP: i32 = 10;
    pub const SECTION_LABEL_H: i32 = 16;
    pub const SIDEBAR_W: i32 = 164;
    pub const PINNED_CARD_H: i32 = 36;
    pub const PINNED_GRID_COL_GAP: i32 = 8;
    pub const PINNED_GRID_ROW_GAP: i32 = 6;
    pub const CARD_RADIUS: i32 = super::radius::LG;
    pub const SIDEBAR_RADIUS: i32 = super::radius::MD;
    pub const SEARCH_RADIUS: i32 = super::radius::MD;
    pub const SIDEBAR_ITEM_RADIUS: i32 = super::radius::SM;
    pub const LIST_ROW_RADIUS: i32 = super::radius::SM;
}

#[cfg(test)]
mod tests {
    use super::{badge, launcher, panel, spacing};

    #[test]
    fn panel_workspace_button_stays_inside_height() {
        assert!(panel::WORKSPACE_BUTTON_Y + panel::WORKSPACE_BUTTON_H <= 42);
    }

    #[test]
    fn launcher_rows_are_taller_than_gaps() {
        assert!(launcher::APP_ROW_H > launcher::ROW_GAP);
        assert!(launcher::PINNED_CARD_H > launcher::PINNED_GRID_ROW_GAP);
    }

    #[test]
    fn badge_is_larger_than_base_spacing() {
        assert!(badge::SIZE > spacing::MD);
        assert!(badge::CONTENT_GAP >= spacing::MD);
    }

    #[test]
    fn surface_radii_follow_shell_hierarchy() {
        assert!(launcher::CARD_RADIUS >= launcher::SEARCH_RADIUS);
        assert!(panel::OUTER_RADIUS >= panel::GROUP_RADIUS);
        assert!(panel::GROUP_RADIUS >= panel::BUTTON_RADIUS);
        assert!(badge::RADIUS <= launcher::LIST_ROW_RADIUS);
    }
}
