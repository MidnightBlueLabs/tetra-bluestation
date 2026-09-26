#[derive(Debug, Clone, Default)]
pub struct GlobalState {
    /// Backhaul/network connection to SwMI (e.g., Brew/TetraPack). False -> fallback mode.
    pub network_connected: bool,
}
