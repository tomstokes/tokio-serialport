use crate::Settings;
use std::path::Path;

pub(crate) struct Port {}

impl Port {
    pub(crate) async fn open(
        _path: impl AsRef<Path>,
        _settings: Settings,
    ) -> std::io::Result<Self> {
        Ok(Self {})
    }
}
