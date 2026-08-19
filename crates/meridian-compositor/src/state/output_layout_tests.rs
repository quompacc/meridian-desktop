use meridian_config::{OutputEntry, OutputModeConfig, OutputPositionConfig};
use smithay::utils::Transform;

use super::{
    detect_output_reload_diff, ConnectedOutput, OutputLayout, OutputPlacement, OutputPosition,
    ResolvedOutput,
};
use crate::state::MeridianState;

include!("output_layout_tests/resolution.rs");
include!("output_layout_tests/config_and_diff.rs");
include!("output_layout_tests/fixtures.rs");
