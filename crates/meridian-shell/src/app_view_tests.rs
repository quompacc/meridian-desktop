use super::*;

#[test]
fn blit_rgba_to_argb_swaps_red_and_blue() {
    let src = [0x12u8, 0x34, 0x56, 0x78];
    let mut dst = [0u8; 4];
    blit_rgba_to_argb(&src, &mut dst);
    assert_eq!(dst, [0x56, 0x34, 0x12, 0x78]);
}

#[test]
fn blit_twice_roundtrips() {
    let src = [0x12u8, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0];
    let mut mid = [0u8; 8];
    let mut dst = [0u8; 8];
    blit_rgba_to_argb(&src, &mut mid);
    blit_rgba_to_argb(&mid, &mut dst);
    assert_eq!(dst, src);
}

#[test]
fn hit_favorite_rows_follow_sidebar_geometry() {
    let n = 8;
    let tile_y = CP_BENTO_TOP + CP_SECTION_LABEL_H;
    let stride = CP_BENTO_TILE_H + CP_BENTO_TILE_GAP;
    assert_eq!(hit_bento_tile(CP_SECTION_PAD + 10, tile_y + 10, n), Some(0));
    assert_eq!(
        hit_bento_tile(CP_SECTION_PAD + 10, tile_y + stride + 10, n),
        Some(1)
    );
    assert_eq!(hit_bento_tile(CP_SECTION_PAD + 10, tile_y - 1, n), None);
    assert_eq!(hit_bento_tile(CP_SECTION_PAD - 1, tile_y + 10, n), None);
}

#[test]
fn hit_app_row_grid_mode() {
    let content_y = CP_APPS_TOP + LAUNCHER_LAYOUT.content_pad + LAUNCHER_LAYOUT.app_heading_height;
    let idx = hit_app_row(CP_GUTTER + 5, content_y + 5, 0, 620, false);
    assert_eq!(idx, Some(0));
    let idx = hit_app_row(
        CP_GUTTER + CP_CARD_W + CP_COL_GAP + 5,
        content_y + 5,
        0,
        620,
        false,
    );
    assert_eq!(idx, Some(1)); // col 1
}

#[test]
fn hit_app_row_search_mode() {
    let content_y = CP_APPS_TOP + LAUNCHER_LAYOUT.content_pad + LAUNCHER_LAYOUT.app_heading_height;
    assert_eq!(
        hit_app_row(CP_GUTTER + 5, content_y + 5, 0, 620, true),
        Some(0)
    );
    assert_eq!(
        hit_app_row(
            CP_GUTTER + CP_CARD_W + CP_COL_GAP + 5,
            content_y + 5,
            0,
            620,
            true
        ),
        Some(1)
    );
}

#[test]
fn hit_footer_power_btn_range() {
    let h = 620u32;
    assert_eq!(hit_footer_power_btn(672, 578, h), Some(0));
    assert_eq!(hit_footer_power_btn(712, 578, h), Some(1));
    assert_eq!(hit_footer_power_btn(672, 550, h), None);
}

#[test]
fn hit_header_settings_btn() {
    assert!(hit_header_settings(636, 578, 880));
    assert!(!hit_header_settings(636, 550, 880));
    assert!(!hit_header_settings(600, 578, 880));
}

#[test]
fn grid_selection_follows_columns_and_rows() {
    let count = CP_APP_COLS * 3 - 1;
    assert_eq!(
        next_grid_selection(None, count, GridDirection::Right),
        Some(0)
    );
    assert_eq!(
        next_grid_selection(None, count, GridDirection::Up),
        Some(count - 1)
    );
    assert_eq!(
        next_grid_selection(Some(1), count, GridDirection::Down),
        Some(1 + CP_APP_COLS)
    );
    assert_eq!(
        next_grid_selection(Some(1 + CP_APP_COLS), count, GridDirection::Up),
        Some(1)
    );
    assert_eq!(
        next_grid_selection(Some(0), count, GridDirection::Left),
        Some(0)
    );
    assert_eq!(
        next_grid_selection(Some(count - 1), count, GridDirection::Right),
        Some(count - 1)
    );
    assert_eq!(next_grid_selection(None, 0, GridDirection::Down), None);
}

#[test]
fn grid_selection_scrolls_only_when_it_leaves_the_view() {
    assert_eq!(scroll_grid_selection_into_view(0, 0, CP_APP_COLS * 8), 0);
    let lower = scroll_grid_selection_into_view(0, CP_APP_COLS * 7, CP_APP_COLS * 8);
    assert!(lower > 0);
    assert_eq!(
        scroll_grid_selection_into_view(lower, 0, CP_APP_COLS * 8),
        0
    );
}
