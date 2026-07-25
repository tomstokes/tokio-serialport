use crate::settings::Settings;
use crate::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::path::{Path, PathBuf};

#[must_use]
#[derive(Clone, Debug)]
pub struct SerialPortBuilder {
    path: PathBuf,
    settings: Settings,
}

impl SerialPortBuilder {
    pub(crate) fn new(path: impl AsRef<Path>, baud_rate: u32) -> Self {
        Self {
            path: path.as_ref().to_owned(),
            settings: Settings::new(baud_rate),
        }
    }

    pub fn baud_rate(mut self, baud_rate: u32) -> Self {
        self.settings.baud_rate = baud_rate;
        self
    }

    pub fn data_bits(mut self, data_bits: DataBits) -> Self {
        self.settings.data_bits = data_bits;
        self
    }

    pub fn flow_control(mut self, flow_control: FlowControl) -> Self {
        self.settings.flow_control = flow_control;
        self
    }

    pub fn parity(mut self, parity: Parity) -> Self {
        self.settings.parity = parity;
        self
    }

    pub fn stop_bits(mut self, stop_bits: StopBits) -> Self {
        self.settings.stop_bits = stop_bits;
        self
    }

    // TODO: Support open options like exclusive mode, etc.
    pub async fn open(self) -> std::io::Result<SerialPort> {
        SerialPort::open_with_settings(self.path, self.settings).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_9600_8_n_1() {
        let builder = SerialPortBuilder::new("/dev/port", 9600);

        assert_eq!(builder.path, Path::new("/dev/port"));
        assert_eq!(
            builder.settings,
            Settings {
                baud_rate: 9600,
                data_bits: DataBits::Eight,
                parity: Parity::None,
                stop_bits: StopBits::One,
                flow_control: FlowControl::None,
            }
        );
    }

    #[test]
    fn configures_serial_options() {
        let builder = SerialPortBuilder::new("/dev/port", 9600)
            .baud_rate(115200)
            .data_bits(DataBits::Seven)
            .flow_control(FlowControl::Hardware)
            .parity(Parity::Even)
            .stop_bits(StopBits::Two);

        assert_eq!(builder.path, Path::new("/dev/port"));
        assert_eq!(
            builder.settings,
            Settings {
                baud_rate: 115200,
                data_bits: DataBits::Seven,
                parity: Parity::Even,
                stop_bits: StopBits::Two,
                flow_control: FlowControl::Hardware,
            }
        );
    }
}
