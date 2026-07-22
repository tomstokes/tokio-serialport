#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod imp;

pub(crate) use imp::Port;
