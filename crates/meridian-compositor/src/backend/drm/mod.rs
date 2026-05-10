use std::{
    collections::{BTreeMap, HashMap},
    time::{Duration, Instant},
};

use smithay::{
    backend::{
        allocator::gbm::GbmAllocator,
        drm::{compositor::DrmCompositor, exporter::gbm::GbmFramebufferExporter, DrmDeviceFd},
        renderer::{element::memory::MemoryRenderBuffer, gles::GlesRenderer},
    },
    output::Output,
    reexports::drm::control::{connector, crtc},
};

use crate::state::OutputId;

use crate::{cursor::CursorImage, wallpaper::WallpaperGpuCache};

mod gpu;
mod init;
mod render;

pub use init::init_drm;
pub use render::{layer_role, render_stack_order, RenderStackRole};

pub type GbmDrmCompositor =
    DrmCompositor<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, (), DrmDeviceFd>;

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct RenderPassMetrics {
    pub rendered_frames: u64,
    pub empty_frames: u64,
    pub outputs_processed: u64,
    pub outputs_skipped_clean: u64,
    pub outputs_skipped_in_flight: u64,
    pub queued_frames: u64,
    pub queue_failures: u64,
    pub rendered_outputs_with_layers: u64,
    pub rendered_outputs_with_space: u64,
    pub rendered_outputs_with_layers_only: u64,
    pub render_elements: u64,
    pub layer_surfaces: u64,
    pub output_pass_duration: Duration,
    pub commit_duration: Duration,
    pub queue_duration: Duration,
}

#[derive(Debug, Clone, Copy, Default)]
struct DurationStats {
    count: u64,
    total_ns: u128,
    min_ns: u64,
    max_ns: u64,
}

impl DurationStats {
    fn record(&mut self, duration: Duration) {
        let nanos = duration.as_nanos() as u64;
        if self.count == 0 {
            self.min_ns = nanos;
            self.max_ns = nanos;
        } else {
            self.min_ns = self.min_ns.min(nanos);
            self.max_ns = self.max_ns.max(nanos);
        }
        self.count += 1;
        self.total_ns += nanos as u128;
    }

    fn avg_ms(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        (self.total_ns as f64 / self.count as f64) / 1_000_000.0
    }

    fn min_ms(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        self.min_ns as f64 / 1_000_000.0
    }

    fn max_ms(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        self.max_ns as f64 / 1_000_000.0
    }
}

#[derive(Debug)]
pub struct DrmTimingStats {
    enabled: bool,
    report_interval: Duration,
    last_report: Instant,
    last_timer_fire: Option<Instant>,
    last_tick: Option<Instant>,
    last_vblank: Option<Instant>,
    ticks: u64,
    frames: u64,
    empty_frames: u64,
    outputs_skipped_clean: u64,
    outputs_skipped_in_flight: u64,
    vblank_events: u64,
    vblank_with_output: u64,
    queue_failures: u64,
    queued_frames_pending: i64,
    rendered_outputs_with_layers: u64,
    rendered_outputs_with_space: u64,
    rendered_outputs_with_layers_only: u64,
    render_elements: u64,
    layer_surfaces: u64,
    timer_fire_interval: DurationStats,
    timer_fire_lag: DurationStats,
    tick_interval: DurationStats,
    render_duration: DurationStats,
    output_pass_duration: DurationStats,
    commit_duration: DurationStats,
    queue_duration: DurationStats,
    vblank_interval: DurationStats,
    vblank_handler_duration: DurationStats,
    frame_submitted_duration: DurationStats,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PerOutputDirtyStats {
    pub dirty_set_count: u64,
    pub dirty_clear_count: u64,
    pub skipped_clean_count: u64,
    pub rendered_dirty_count: u64,
    pub rendered_while_not_dirty_count: u64,
}

#[derive(Debug)]
pub struct DrmDirtyStats {
    enabled: bool,
    report_interval: Duration,
    last_report: Instant,
    reasons: BTreeMap<String, u64>,
    per_output: HashMap<OutputId, PerOutputDirtyStats>,
    output_names: HashMap<OutputId, String>,
}

impl DrmDirtyStats {
    pub fn new(enabled: bool) -> Self {
        let now = Instant::now();
        if enabled {
            tracing::info!("drm dirty stats enabled: report_interval_ms=1000");
        }
        Self {
            enabled,
            report_interval: Duration::from_secs(1),
            last_report: now,
            reasons: BTreeMap::new(),
            per_output: HashMap::new(),
            output_names: HashMap::new(),
        }
    }

    pub fn register_output(&mut self, output_id: OutputId, output_name: String) {
        if !self.enabled {
            return;
        }
        self.output_names.insert(output_id, output_name);
        self.per_output.entry(output_id).or_default();
    }

    pub fn unregister_output(&mut self, output_id: OutputId) {
        if !self.enabled {
            return;
        }
        self.output_names.remove(&output_id);
        self.per_output.remove(&output_id);
    }

    pub fn record_dirty_mark_event(&mut self, output_id: OutputId, reason: &str) {
        if !self.enabled {
            return;
        }
        *self.reasons.entry(reason.to_string()).or_insert(0) += 1;
        self.per_output.entry(output_id).or_default();
    }

    pub fn record_dirty_set(&mut self, output_id: OutputId) {
        if !self.enabled {
            return;
        }
        self.per_output
            .entry(output_id)
            .or_default()
            .dirty_set_count += 1;
    }

    pub fn record_dirty_clear(&mut self, output_id: OutputId) {
        if !self.enabled {
            return;
        }
        self.per_output
            .entry(output_id)
            .or_default()
            .dirty_clear_count += 1;
    }

    pub fn record_skipped_clean(&mut self, output_id: OutputId) {
        if !self.enabled {
            return;
        }
        self.per_output
            .entry(output_id)
            .or_default()
            .skipped_clean_count += 1;
    }

    pub fn record_rendered_dirty(&mut self, output_id: OutputId) {
        if !self.enabled {
            return;
        }
        self.per_output
            .entry(output_id)
            .or_default()
            .rendered_dirty_count += 1;
    }

    pub fn record_rendered_while_not_dirty(&mut self, output_id: OutputId) {
        if !self.enabled {
            return;
        }
        self.per_output
            .entry(output_id)
            .or_default()
            .rendered_while_not_dirty_count += 1;
    }

    pub fn report_if_due(&mut self, now: Instant) {
        if !self.enabled || now.saturating_duration_since(self.last_report) < self.report_interval {
            return;
        }

        let mut reason_summary = String::new();
        for (idx, (reason, count)) in self.reasons.iter().enumerate() {
            if idx > 0 {
                reason_summary.push_str(", ");
            }
            reason_summary.push_str(reason);
            reason_summary.push('=');
            reason_summary.push_str(&count.to_string());
        }
        if reason_summary.is_empty() {
            reason_summary.push_str("<none>");
        }

        tracing::info!("drm dirty reasons (1s): {}", reason_summary);

        let mut ids: Vec<_> = self.per_output.keys().copied().collect();
        ids.sort_by_key(|id| id.0);
        for output_id in ids {
            if let Some(stats) = self.per_output.get(&output_id) {
                let output_name = self
                    .output_names
                    .get(&output_id)
                    .map_or("<unknown>", String::as_str);
                tracing::info!(
                    "drm dirty output stats (1s): output_id={} output={} dirty_set_count={} dirty_clear_count={} skipped_clean_count={} rendered_dirty_count={} rendered_while_not_dirty_count={}",
                    output_id.0,
                    output_name,
                    stats.dirty_set_count,
                    stats.dirty_clear_count,
                    stats.skipped_clean_count,
                    stats.rendered_dirty_count,
                    stats.rendered_while_not_dirty_count
                );
            }
        }

        self.last_report = now;
        self.reasons.clear();
        for stats in self.per_output.values_mut() {
            *stats = PerOutputDirtyStats::default();
        }
    }
}

impl DrmTimingStats {
    pub fn new(enabled: bool) -> Self {
        let now = Instant::now();
        let stats = Self {
            enabled,
            report_interval: Duration::from_secs(1),
            last_report: now,
            last_timer_fire: None,
            last_tick: None,
            last_vblank: None,
            ticks: 0,
            frames: 0,
            empty_frames: 0,
            outputs_skipped_clean: 0,
            outputs_skipped_in_flight: 0,
            vblank_events: 0,
            vblank_with_output: 0,
            queue_failures: 0,
            queued_frames_pending: 0,
            rendered_outputs_with_layers: 0,
            rendered_outputs_with_space: 0,
            rendered_outputs_with_layers_only: 0,
            render_elements: 0,
            layer_surfaces: 0,
            timer_fire_interval: DurationStats::default(),
            timer_fire_lag: DurationStats::default(),
            tick_interval: DurationStats::default(),
            render_duration: DurationStats::default(),
            output_pass_duration: DurationStats::default(),
            commit_duration: DurationStats::default(),
            queue_duration: DurationStats::default(),
            vblank_interval: DurationStats::default(),
            vblank_handler_duration: DurationStats::default(),
            frame_submitted_duration: DurationStats::default(),
        };
        if enabled {
            tracing::info!(
                "drm timing aggregation enabled: report_interval_ms={}",
                stats.report_interval.as_millis()
            );
            tracing::info!(
                "drm render schedule diagnostics enabled: timer-driven scheduling (interval configured in drm init)"
            );
        }
        stats
    }

    pub(super) fn record_render_tick(
        &mut self,
        timer_fired_at: Instant,
        tick_started: Instant,
        render_duration: Duration,
        metrics: RenderPassMetrics,
    ) {
        if !self.enabled {
            return;
        }

        if let Some(last_timer_fire) = self.last_timer_fire {
            self.timer_fire_interval
                .record(timer_fired_at.saturating_duration_since(last_timer_fire));
        }
        self.last_timer_fire = Some(timer_fired_at);
        self.timer_fire_lag
            .record(tick_started.saturating_duration_since(timer_fired_at));

        if let Some(last_tick) = self.last_tick {
            self.tick_interval
                .record(tick_started.saturating_duration_since(last_tick));
        }
        self.last_tick = Some(tick_started);

        self.ticks += 1;
        self.frames += metrics.rendered_frames;
        self.empty_frames += metrics.empty_frames;
        self.outputs_skipped_clean += metrics.outputs_skipped_clean;
        self.outputs_skipped_in_flight += metrics.outputs_skipped_in_flight;
        self.queue_failures += metrics.queue_failures;
        self.queued_frames_pending += metrics.queued_frames as i64;
        self.rendered_outputs_with_layers += metrics.rendered_outputs_with_layers;
        self.rendered_outputs_with_space += metrics.rendered_outputs_with_space;
        self.rendered_outputs_with_layers_only += metrics.rendered_outputs_with_layers_only;
        self.render_elements += metrics.render_elements;
        self.layer_surfaces += metrics.layer_surfaces;
        self.render_duration.record(render_duration);
        if metrics.outputs_processed > 0 {
            self.output_pass_duration.record(Duration::from_nanos(
                (metrics.output_pass_duration.as_nanos() / metrics.outputs_processed as u128)
                    as u64,
            ));
        }
        if metrics.rendered_frames > 0 {
            self.commit_duration.record(metrics.commit_duration);
            self.queue_duration.record(metrics.queue_duration);
        }

        self.report_if_due(tick_started);
    }

    pub(super) fn record_vblank(
        &mut self,
        at: Instant,
        handler_duration: Duration,
        frame_submitted_duration: Duration,
        matched_output: bool,
    ) {
        if !self.enabled {
            return;
        }
        self.vblank_events += 1;
        self.vblank_handler_duration.record(handler_duration);
        if matched_output {
            self.vblank_with_output += 1;
            self.frame_submitted_duration
                .record(frame_submitted_duration);
            if self.queued_frames_pending > 0 {
                self.queued_frames_pending -= 1;
            }
        }
        if let Some(last) = self.last_vblank {
            self.vblank_interval
                .record(at.saturating_duration_since(last));
        }
        self.last_vblank = Some(at);
    }

    fn report_if_due(&mut self, now: Instant) {
        if now.saturating_duration_since(self.last_report) < self.report_interval {
            return;
        }

        let avg_render_elements = if self.frames == 0 {
            0.0
        } else {
            self.render_elements as f64 / self.frames as f64
        };
        let avg_layer_surfaces = if self.frames == 0 {
            0.0
        } else {
            self.layer_surfaces as f64 / self.frames as f64
        };

        tracing::info!(
            "drm timing summary: ticks={} frames={} empty_frames={} outputs_skipped_clean={} outputs_skipped_in_flight={} vblank_events={} vblank_with_output={} queued_pending={} queue_failures={} timer_fire_ms(avg/min/max)={:.2}/{:.2}/{:.2} timer_lag_ms(avg/min/max)={:.2}/{:.2}/{:.2} tick_ms(avg/min/max)={:.2}/{:.2}/{:.2} render_ms(avg/min/max)={:.2}/{:.2}/{:.2} output_pass_ms(avg/min/max)={:.2}/{:.2}/{:.2} commit_ms(avg/min/max)={:.2}/{:.2}/{:.2} queue_ms(avg/min/max)={:.2}/{:.2}/{:.2} vblank_wait_ms(avg/min/max)={:.2}/{:.2}/{:.2} vblank_handler_ms(avg/min/max)={:.2}/{:.2}/{:.2} frame_submitted_ms(avg/min/max)={:.2}/{:.2}/{:.2} render_elements_per_frame_avg={:.1} layer_surfaces_per_frame_avg={:.1}",
            self.ticks,
            self.frames,
            self.empty_frames,
            self.outputs_skipped_clean,
            self.outputs_skipped_in_flight,
            self.vblank_events,
            self.vblank_with_output,
            self.queued_frames_pending,
            self.queue_failures,
            self.timer_fire_interval.avg_ms(),
            self.timer_fire_interval.min_ms(),
            self.timer_fire_interval.max_ms(),
            self.timer_fire_lag.avg_ms(),
            self.timer_fire_lag.min_ms(),
            self.timer_fire_lag.max_ms(),
            self.tick_interval.avg_ms(),
            self.tick_interval.min_ms(),
            self.tick_interval.max_ms(),
            self.render_duration.avg_ms(),
            self.render_duration.min_ms(),
            self.render_duration.max_ms(),
            self.output_pass_duration.avg_ms(),
            self.output_pass_duration.min_ms(),
            self.output_pass_duration.max_ms(),
            self.commit_duration.avg_ms(),
            self.commit_duration.min_ms(),
            self.commit_duration.max_ms(),
            self.queue_duration.avg_ms(),
            self.queue_duration.min_ms(),
            self.queue_duration.max_ms(),
            self.vblank_interval.avg_ms(),
            self.vblank_interval.min_ms(),
            self.vblank_interval.max_ms(),
            self.vblank_handler_duration.avg_ms(),
            self.vblank_handler_duration.min_ms(),
            self.vblank_handler_duration.max_ms(),
            self.frame_submitted_duration.avg_ms(),
            self.frame_submitted_duration.min_ms(),
            self.frame_submitted_duration.max_ms(),
            avg_render_elements,
            avg_layer_surfaces
        );
        tracing::info!(
            "drm repaint mix: rendered_outputs_with_layers={} rendered_outputs_with_space={} rendered_outputs_with_layers_only={}",
            self.rendered_outputs_with_layers,
            self.rendered_outputs_with_space,
            self.rendered_outputs_with_layers_only
        );

        self.last_report = now;
        self.ticks = 0;
        self.frames = 0;
        self.empty_frames = 0;
        self.outputs_skipped_clean = 0;
        self.outputs_skipped_in_flight = 0;
        self.vblank_events = 0;
        self.vblank_with_output = 0;
        self.queue_failures = 0;
        self.rendered_outputs_with_layers = 0;
        self.rendered_outputs_with_space = 0;
        self.rendered_outputs_with_layers_only = 0;
        self.render_elements = 0;
        self.layer_surfaces = 0;
        self.timer_fire_interval = DurationStats::default();
        self.timer_fire_lag = DurationStats::default();
        self.tick_interval = DurationStats::default();
        self.render_duration = DurationStats::default();
        self.output_pass_duration = DurationStats::default();
        self.commit_duration = DurationStats::default();
        self.queue_duration = DurationStats::default();
        self.vblank_interval = DurationStats::default();
        self.vblank_handler_duration = DurationStats::default();
        self.frame_submitted_duration = DurationStats::default();
    }
}

pub struct DrmOutput {
    pub output_id: OutputId,
    pub output: Output,
    pub compositor: GbmDrmCompositor,
    pub crtc: crtc::Handle,
    pub connector: connector::Handle,
    pub wallpaper: Option<WallpaperGpuCache>,
    pub frame_in_flight: bool,
    pub needs_repaint: bool,
}

pub struct DrmBackend {
    pub device_fd: DrmDeviceFd,
    pub kms_node_path: String,
    pub kms_is_primary_node: bool,
    pub kms_master_lock_ok: bool,
    pub kms_first_commit_verified: bool,
    pub renderer: GlesRenderer,
    pub outputs: Vec<DrmOutput>,
    pub cursor_image: CursorImage,
    pub cursor_buffer: MemoryRenderBuffer,
    pub dirty_stats: DrmDirtyStats,
    pub last_pointer_location: Option<(f64, f64)>,
    pub last_connector_scan: Instant,
    pub timing_stats: DrmTimingStats,
}
