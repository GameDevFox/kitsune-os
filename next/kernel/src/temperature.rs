//! The chip's temperature, from its own sensor, checked against the firmware's reading.

use crate::board::{self, thermal};
use crate::devices;

const THOUSANDTHS: i32 = 1000;

/// The temperature in thousandths of a degree Celsius, or `None` if the device tree has
/// no sensor or it isn't ready. An optional device, so there's no fallback without a tree.
pub fn millidegrees() -> Option<i32> {
    let base = devices::address_of("avs_monitor")?;
    let (slope, offset) = devices::thermal_conversion()?;
    let raw = thermal::read_raw(base + thermal::STATUS_OFFSET)?;
    Some(slope * raw as i32 + offset)
}

/// Prints the sensor's reading beside the firmware's.
pub fn check() {
    match (millidegrees(), board::firmware_temperature()) {
        (Some(ours), Some(firmware)) => println!(
            "Temperature: {}.{:03} C by the sensor, {}.{:03} C by the firmware.",
            ours / THOUSANDTHS,
            ours % THOUSANDTHS,
            firmware / THOUSANDTHS as u32,
            firmware % THOUSANDTHS as u32
        ),
        (ours, firmware) => println!("Temperature: sensor {ours:?}, firmware {firmware:?}, in thousandths of a degree."),
    }
}
