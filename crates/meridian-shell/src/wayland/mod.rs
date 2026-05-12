mod calendar;
mod handlers;
mod init;
mod ipc;
mod render;
mod shell;
mod state;
mod time;
mod types;

pub use ipc::IpcClient;
pub use types::{ClickAction, ClickZone, Rect};

pub(crate) use init::initialize;
pub(crate) use shell::{
    CommitReason, CommitStats, CommitSurfaceKind, MeridianShell, RepaintReason,
};
pub(crate) use types::SurfaceKind;
