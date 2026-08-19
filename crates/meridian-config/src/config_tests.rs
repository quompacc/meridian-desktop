use std::{fs, time::SystemTime};

use super::{set_output_mode_in_toml, set_primary_output_in_toml, MeridianConfig, WallpaperMode};
use crate::{OutputModeConfig, OutputPositionConfig};

include!("config_tests/parsing.rs");
include!("config_tests/output_updates.rs");
include!("config_tests/wallpaper_and_idle.rs");
