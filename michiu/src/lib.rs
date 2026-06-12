
#[cfg(feature = "guard")]
pub use michiu_guard::{Unvalidated, Validate, Validated};

#[cfg(feature = "window")]
pub use michiu_window as window;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}
