#[cfg(feature = "log")]
pub(crate) use log::{debug, error};

#[cfg(not(feature = "log"))]
macro_rules! _debug {
    (target: $target:expr, $($arg:tt)+) => {};
    ($($arg:tt)+) => {};
}

#[cfg(not(feature = "log"))]
macro_rules! _error {
    (target: $target:expr, $($arg:tt)+) => {};
    ($($arg:tt)+) => {};
}

#[cfg(not(feature = "log"))]
pub(crate) use {_debug as debug, _error as error};
