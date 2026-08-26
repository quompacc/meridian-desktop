impl MeridianShell {
    pub(crate) fn draw_calendar_popup(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        debug!(
            "draw_calendar_popup: reason={:?} open={} configured={} calendar_dirty={} commit_expected={}",
            reason,
            self.calendar_popup_open,
            self.calendar_configured,
            self.calendar_dirty,
            self.calendar_popup_open && self.calendar_configured
        );
        if !self.calendar_popup_open || !self.calendar_configured {
            return;
        }

        let surface_w = self.calendar_width;
        let surface_h = self.calendar_height;
        let card_w = CALENDAR_POPUP_WIDTH;
        let card_h = CALENDAR_POPUP_HEIGHT;

        // Render the card into its own temp buffer at card-natural size.
        let mut card_buf = vec![0u8; (card_w as usize) * (card_h as usize) * 4];
        {
            let mut painter = Painter::new(&mut card_buf, card_w as i32, card_h as i32);
            crate::popup_card::draw_card_body(&mut painter, &self.theme);
            let card = Rect {
                x: 0,
                y: 0,
                w: card_w as i32,
                h: card_h as i32,
            };

            let maybe_model = time::local_date().and_then(|date| {
                CalendarMonthModel::for_month(
                    date.year,
                    date.month,
                    Some(date.day),
                    self.calendar_display_policy.week_start,
                )
            });

            if let Some(model) = maybe_model {
                let labels = weekday_labels(self.calendar_display_policy.week_start);
                let header_text = format!("{} {}", german_month_name(model.month), model.year);
                crate::popup_card::draw_card_title(
                    &mut painter,
                    &self.font,
                    &self.theme,
                    &header_text,
                );
                let content = Rect {
                    x: card.x + crate::popup_card::PAD_X,
                    y: crate::popup_card::BODY_TOP,
                    w: card.w - 2 * crate::popup_card::PAD_X,
                    h: (card.h - crate::popup_card::BODY_TOP - crate::popup_card::PAD_BOTTOM)
                        .max(1),
                };
                let layout = meridian_tokens::Calendar::DEFAULT;
                let weekday_y = content.y;
                let weekday_h = layout.weekday_height;
                for (col, label) in labels.iter().enumerate() {
                    let col = col as i32;
                    let cells_w = content.w - (layout.columns - 1) * layout.cell_gap;
                    let x0 = content.x
                        + (col * cells_w) / layout.columns
                        + col * layout.cell_gap;
                    let x1 = content.x
                        + ((col + 1) * cells_w) / layout.columns
                        + col * layout.cell_gap;
                    painter.text_centered(
                        &self.font,
                        label,
                        Rect {
                            x: x0,
                            y: weekday_y,
                            w: x1 - x0,
                            h: weekday_h,
                        },
                        crate::ui::tokens::glass_dim_from_config(&self.theme),
                    );
                }
                let grid_y = weekday_y + weekday_h + layout.weekday_gap;
                let grid_h = (content.y + content.h) - grid_y;
                let cells_h = grid_h - (layout.rows - 1) * layout.cell_gap;
                let cells_w = content.w - (layout.columns - 1) * layout.cell_gap;
                for row in 0..layout.rows as usize {
                    let row_i32 = row as i32;
                    let y0 = grid_y
                        + (row_i32 * cells_h) / layout.rows
                        + row_i32 * layout.cell_gap;
                    let y1 = grid_y
                        + ((row_i32 + 1) * cells_h) / layout.rows
                        + row_i32 * layout.cell_gap;
                    for col in 0..layout.columns as usize {
                        let idx = row * layout.columns as usize + col;
                        let Some(day) = model.cells[idx] else {
                            continue;
                        };
                        let col_i32 = col as i32;
                        let x0 = content.x
                            + (col_i32 * cells_w) / layout.columns
                            + col_i32 * layout.cell_gap;
                        let x1 = content.x
                            + ((col_i32 + 1) * cells_w) / layout.columns
                            + col_i32 * layout.cell_gap;
                        let cell_rect = Rect {
                            x: x0,
                            y: y0,
                            w: x1 - x0,
                            h: y1 - y0,
                        };
                        let is_today = model.today_day == Some(day);
                        let day_text = day.to_string();
                        if is_today {
                            let inset = layout.today_inset;
                            let highlight = Rect {
                                x: cell_rect.x + inset,
                                y: cell_rect.y + inset,
                                w: (cell_rect.w - 2 * inset).max(0),
                                h: (cell_rect.h - 2 * inset).max(0),
                            };
                            if highlight.w > 0 && highlight.h > 0 {
                                painter.roundish_rect_with_radius(
                                    highlight,
                                    self.theme.colors.accent,
                                    meridian_tokens::Radius::DEFAULT.sm,
                                );
                            }
                            painter.text_centered(
                                &self.font,
                                &day_text,
                                cell_rect,
                                crate::ui::tokens::accent_foreground_from_config(&self.theme),
                            );
                        } else {
                            painter.text_centered(
                                &self.font,
                                &day_text,
                                cell_rect,
                                crate::ui::tokens::glass_foreground_from_config(&self.theme),
                            );
                        }
                    }
                }
            } else {
                let time_text = if self.last_clock.is_empty() {
                    time::formatted_time()
                } else {
                    self.last_clock.clone()
                };
                let text_rect = Rect {
                    x: card.x + 12,
                    y: card.y + 16,
                    w: card.w - 24,
                    h: 28,
                };
                painter.text_centered(
                    &self.font,
                    &time_text,
                    text_rect,
                    crate::ui::tokens::glass_foreground_from_config(&self.theme),
                );
            }
        }
        round_buffer_corners(
            &mut card_buf,
            card_w as usize,
            card_h as usize,
            crate::ui::tokens::surface_radius_from_config(
                &self.theme,
                meridian_config::ThemeSurface::Popup,
            ),
        );

        // Now obtain the SHM surface buffer and composite card + shadow into it.
        let stride = buffer::shm_buffer_stride(surface_w);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.calendar_buffer,
                surface_w,
                surface_h,
                stride,
            );
            let Some(buf) = buf else {
                warn!("calendar popup buffer unavailable: reason={:?}", reason);
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.calendar_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!("calendar popup canvas unavailable after retry");
                return;
            };
            crate::popup_card::paint_card_with_shadow(
                canvas,
                surface_w,
                surface_h,
                card_w,
                card_h,
                &card_buf,
                crate::ui::tokens::surface_radius_from_config(
                    &self.theme,
                    meridian_config::ThemeSurface::Popup,
                ),
            );
            if let Err(err) = buf.attach_to(self.calendar_layer.wl_surface()) {
                warn!("calendar popup buffer attach failed: {}", err);
                return;
            }
            self.calendar_layer.wl_surface().damage_buffer(
                0,
                0,
                surface_w as i32,
                surface_h as i32,
            );
            self.calendar_layer.commit();
            self.calendar_dirty = false;
            return;
        }
    }

    pub(crate) fn unmap_calendar_popup(&mut self, reason: CommitReason) {
        debug!(
            "unmap_calendar_popup: reason={:?} open={} configured={} surface=calendar attach_none=true commit=true",
            reason, self.calendar_popup_open, self.calendar_configured
        );
        self.calendar_layer.wl_surface().attach(None, 0, 0);
        self.calendar_layer.commit();
        self.calendar_dirty = false;
    }

    pub(crate) fn draw_workspace_popup(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        if !self.workspace_popup_open || !self.workspace_configured {
            return;
        }
        let surface_w = self.workspace_width;
        let surface_h = self.workspace_height;
        let card_w = WORKSPACE_POPUP_WIDTH;
        let card_h = WORKSPACE_POPUP_HEIGHT;
        let active_workspace = self.panel_active_workspace() as u32;

        let mut card_buf = vec![0u8; (card_w as usize) * (card_h as usize) * 4];
        {
            let mut painter = Painter::new(&mut card_buf, card_w as i32, card_h as i32);
            workspaces::draw_workspace_popup(
                &mut painter,
                &self.font,
                &self.theme,
                workspaces::WorkspacePopupInput {
                    active_workspace,
                    total_workspaces: 9,
                    occupied: self.occupied_workspaces,
                    hovered_idx: self.workspace_hover_idx,
                },
                &mut self.workspace_state,
            );
        }
        round_buffer_corners(
            &mut card_buf,
            card_w as usize,
            card_h as usize,
            crate::ui::tokens::surface_radius_from_config(
                &self.theme,
                meridian_config::ThemeSurface::Popup,
            ),
        );

        let stride = buffer::shm_buffer_stride(surface_w);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.workspace_buffer,
                surface_w,
                surface_h,
                stride,
            );
            let Some(buf) = buf else {
                warn!("workspace popup buffer unavailable: reason={:?}", reason);
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.workspace_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!("workspace popup canvas unavailable after retry");
                return;
            };
            crate::popup_card::paint_card_with_shadow(
                canvas,
                surface_w,
                surface_h,
                card_w,
                card_h,
                &card_buf,
                crate::ui::tokens::surface_radius_from_config(
                    &self.theme,
                    meridian_config::ThemeSurface::Popup,
                ),
            );
            if let Err(err) = buf.attach_to(self.workspace_layer.wl_surface()) {
                warn!("workspace popup buffer attach failed: {}", err);
                return;
            }
            self.workspace_layer.wl_surface().damage_buffer(
                0,
                0,
                surface_w as i32,
                surface_h as i32,
            );
            self.workspace_layer.commit();
            self.workspace_dirty = false;
            return;
        }
    }

    pub(crate) fn unmap_workspace_popup(&mut self, reason: CommitReason) {
        debug!(
            "unmap_workspace_popup: reason={:?} open={} configured={} surface=workspace attach_none=true commit=true",
            reason, self.workspace_popup_open, self.workspace_configured
        );
        self.workspace_layer.wl_surface().attach(None, 0, 0);
        self.workspace_layer.commit();
        self.workspace_dirty = false;
    }
}
