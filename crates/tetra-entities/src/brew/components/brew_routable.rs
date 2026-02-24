use tetra_config::SharedConfig;

/// Returns true if the Brew component is active
#[inline]
pub fn is_active(config: &SharedConfig) -> bool {
    config.config().brew.is_some()
}

/// Determine if a given SSI should be routed over brew, or is restricted to local handling
pub fn is_brew_routable(config: &SharedConfig, ssi: u32) -> bool {
    let Some(brew_config) = &config.config().brew else {
        // Brew not configured, so no routing to Brew
        return false;
    };
    if ssi <= 90 {
        // Brew doesn't route 0..=90
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
