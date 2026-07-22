#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataBits {
    Five,
    Six,
    Seven,
    Eight,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Parity {
    None,
    Odd,
    Even,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopBits {
    One,
    Two,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlowControl {
    None,
    Software,
    Hardware,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Settings {
    // TODO: Consider a builder pattern or other convenience constructions
    pub(crate) baud_rate: u32,
    pub(crate) data_bits: DataBits,
    pub(crate) parity: Parity,
    pub(crate) stop_bits: StopBits,
    pub(crate) flow_control: FlowControl,
}

impl Settings {
    pub const fn baud_rate(&self) -> u32 {
        self.baud_rate
    }

    pub const fn data_bits(&self) -> DataBits {
        self.data_bits
    }

    pub const fn parity(&self) -> Parity {
        self.parity
    }

    pub const fn stop_bits(&self) -> StopBits {
        self.stop_bits
    }

    pub const fn flow_control(&self) -> FlowControl {
        self.flow_control
    }
}
