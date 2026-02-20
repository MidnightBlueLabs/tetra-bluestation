// At some point, these functions might be exposed through the rust-soapysdr crate.
// For now, we re-implement them here.

const NS_PER_SEC_I64: i64 = 1_000_000_000;
const NS_PER_SEC_F64: f64 = 1_000_000_000.0;

/// Port of SoapySDR_ticksToTimeNs(ticks, rate).
/// `rate` is in ticks/second (e.g., samples/sec).
pub fn ticks_to_time_ns(ticks: i64, rate: f64) -> i64 {
    assert!(rate.is_finite() && rate != 0.0, "rate must be finite and non-zero");

    // C: (long long)(rate) truncates toward zero.
    let ratell: i64 = rate.trunc() as i64;
    assert!(ratell != 0, "rate must have |trunc(rate)| >= 1 for this algorithm");

    // C integer division truncates toward zero; Rust matches for i64.
    let full: i64 = ticks / ratell;
    let err: i64 = ticks - (full * ratell);

    let part: f64 = (full as f64) * (rate - ratell as f64);
    let frac: f64 = (((err as f64) - part) * NS_PER_SEC_F64) / rate;

    // C llround(): nearest, ties away from zero. Rust round() matches for finite values.
    let rounded: i64 = frac.round() as i64;

    let out: i128 = (full as i128) * (NS_PER_SEC_I64 as i128) + (rounded as i128);
    i64::try_from(out).expect("ticks_to_time_ns overflowed i64")
}

/// Port of SoapySDR_timeNsToTicks(timeNs, rate).
/// `rate` is in ticks/second (e.g., samples/sec).
pub fn time_ns_to_ticks(time_ns: i64, rate: f64) -> i64 {
    assert!(rate.is_finite() && rate != 0.0, "rate must be finite and non-zero");

    let ratell: i64 = rate.trunc() as i64;
    assert!(ratell != 0, "rate must have |trunc(rate)| >= 1 for this algorithm");

    let full: i64 = time_ns / NS_PER_SEC_I64;
    let err: i64 = time_ns - (full * NS_PER_SEC_I64);

    let part: f64 = (full as f64) * (rate - ratell as f64);
    let frac: f64 = part + ((err as f64) * rate) / NS_PER_SEC_F64;

    let rounded: i64 = frac.round() as i64;

    let out: i128 = (full as i128) * (ratell as i128) + (rounded as i128);
    i64::try_from(out).expect("time_ns_to_ticks overflowed i64")
}
