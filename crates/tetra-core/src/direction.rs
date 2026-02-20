#[derive(Debug, Clone, PartialEq, Copy)]
pub enum Direction {
    None,
    /// Uplink
    Ul,
    /// Downlink
    Dl,
    Both,
}

impl Direction {
    #[inline]
    pub fn includes_ul(&self) -> bool {
        matches!(self, Direction::Ul | Direction::Both)
    }

    #[inline]
    pub fn includes_dl(&self) -> bool {
        matches!(self, Direction::Dl | Direction::Both)
    }
}
