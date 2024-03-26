#[derive(Clone, Copy)]
pub enum Interrupt {
    VBlank = 0,
    LCD = 1,
    Timer = 2,
    Serial = 3,
    Joypad = 4,
}

impl From<usize> for Interrupt {
    fn from(value: usize) -> Self {
        match value {
            0 => Self::VBlank,
            1 => Self::LCD,
            2 => Self::Timer,
            3 => Self::Serial,
            4 => Self::Joypad,
            _ => panic!("Invalid value for Interrupt: {value}"),
        }
    }
}
