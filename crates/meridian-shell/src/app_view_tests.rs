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
fn hit_bento_tile_correct_columns() {
    // 8 tiles centered in LAUNCHER_WIDTH; stride = tile width + gap.
    let n = 8;
    let tile_y = CP_BENTO_TOP + CP_SECTION_LABEL_H + CP_SECTION_PAD;
    let strip_x = cp_bento_tile_x(n);
    let stride = CP_BENTO_TILE_W + CP_BENTO_TILE_GAP;
    assert_eq!(hit_bento_tile(strip_x + 10, tile_y + 10, n), Some(0));
    assert_eq!(
        hit_bento_tile(strip_x + stride + 10, tile_y + 10, n),
        Some(1)
    );
    assert_eq!(hit_bento_tile(strip_x + 10, tile_y - 1, n), None); // wrong row
    assert_eq!(hit_bento_tile(strip_x - 1, tile_y + 10, n), None); // before strip
}

#[test]
fn hit_app_row_grid_mode() {
    // content_y = 190+24+8=222; col0: x in [21,295)
    let idx = hit_app_row(CP_GUTTER + 5, 222 + 5, 0, 620, false);
    assert_eq!(idx, Some(0));
    let idx = hit_app_row(
        CP_GUTTER + CP_CARD_W + CP_COL_GAP + 5,
        222 + 5,
        0,
        620,
        false,
    );
    assert_eq!(idx, Some(1)); // col 1
}

#[test]
fn hit_app_row_search_mode() {
    // content_y = 53
    assert_eq!(hit_app_row(100, 53 + 5, 0, 620, true), Some(0));
    assert_eq!(hit_app_row(100, 53 + 44 + 5, 0, 620, true), Some(1));
}

#[test]
fn hit_footer_power_btn_range() {
    // btn 0: x=[676,708), y=[footer+4, footer+36); footer=580
    let h = 620u32;
    assert_eq!(hit_footer_power_btn(680, 588, h), Some(0));
    assert_eq!(hit_footer_power_btn(720, 588, h), Some(1)); // 676+40=716..748
    assert_eq!(hit_footer_power_btn(680, 570, h), None); // above footer
}

#[test]
fn hit_header_settings_btn() {
    // w=880: x=[840,868), y=[12,40)
    assert!(hit_header_settings(850, 20, 880));
    assert!(!hit_header_settings(850, 5, 880));
    assert!(!hit_header_settings(800, 20, 880));
}
