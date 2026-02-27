use tetra_config::bluestation::SharedConfig;

/// Returns true if the Brew component is active
#[inline]
pub fn is_active(config: &SharedConfig) -> bool {
    config.config().brew.is_some()
}

/// Returns true if the configured Brew server is TetraPack (core.tetrapack.online)
fn is_tetrapack(config: &SharedConfig) -> bool {
    let Some(brew_config) = &config.config().brew else {
        return false;
    };
    brew_config.host == "core.tetrapack.online"
}

/// Determine if a given GSSI should be routed over Brew, or is restricted to local handling
pub fn is_brew_gssi_routable(config: &SharedConfig, ssi: u32) -> bool {
    let Some(brew_config) = &config.config().brew else {
        // Brew not configured, so no routing to Brew
        return false;
    };
    if is_tetrapack(config) && ssi <= 90 {
        // TetraPack doesn't route 0..=90
        return false;
    }
    if config.config().cell.local_ssi_ranges.contains(ssi) {
        // Range overridden as local
        return false;
    }

    // Check if whitelist is present and if so, check
    if let Some(whitelist) = &brew_config.whitelisted_ssis {
        if whitelist.contains(&ssi) {
            // Range explicitly whitelisted for routing to Brew
            return true;
        } else {
            // Not in whitelist - block routing to Brew
            return false;
        }
    }

    // No whitelist present, default to allow
    true
}

/// Determine if a given ISSI should be sent to the Brew server.
/// On TetraPack, ISSIs must be exactly 7 digits (1_000_000..=9_999_999). Other servers allow all ISSIs.
pub fn is_brew_issi_routable(config: &SharedConfig, issi: u32) -> bool {
    if config.config().brew.is_none() {
        return false;
    }
    if is_tetrapack(config) {
        issi >= 1_000_000 && issi <= 9_999_999
    } else {
        true
    }
}
