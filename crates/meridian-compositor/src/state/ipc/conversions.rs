const WORKSPACE_COUNT: usize = 9;
const MAX_WORKSPACE_INDEX: u8 = (WORKSPACE_COUNT - 1) as u8;

pub(super) fn ipc_workspace_to_index(workspace: u8) -> usize {
    usize::from(workspace.saturating_sub(1).min(MAX_WORKSPACE_INDEX))
}

pub(super) fn index_to_legacy_ipc_workspace(index: usize) -> u8 {
    (index + 1) as u8
}

#[cfg(test)]
mod tests {
    use super::{index_to_legacy_ipc_workspace, ipc_workspace_to_index};

    #[test]
    fn ipc_workspace_to_index_clamps_and_normalizes() {
        assert_eq!(ipc_workspace_to_index(0), 0);
        assert_eq!(ipc_workspace_to_index(1), 0);
        assert_eq!(ipc_workspace_to_index(9), 8);
        assert_eq!(ipc_workspace_to_index(10), 8);
        assert_eq!(ipc_workspace_to_index(255), 8);
    }

    #[test]
    fn index_to_legacy_ipc_workspace_is_one_based() {
        assert_eq!(index_to_legacy_ipc_workspace(0), 1);
        assert_eq!(index_to_legacy_ipc_workspace(8), 9);
    }
}
