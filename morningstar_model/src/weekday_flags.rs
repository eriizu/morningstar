use bitflags::bitflags;
use jiff::civil::{Date, Weekday};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

bitflags! {
    #[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
    pub struct WeekdayFlags: u8 {
        const NEVER = 0;
        const MONDAY =    0b1;
        const TUESDAY =   0b1 << 1;
        const WEDNESDAY = 0b1 << 2;
        const THURSDAY =  0b1 << 3;
        const FRIDAY =    0b1 << 4;
        const SATURDAY =  0b1 << 5;
        const SUNDAY =    0b1 << 6;

        const WORKDAYS = 0b11111;
        const WEEKENDS = 0b11 << 5;
    }
}

impl Display for WeekdayFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if *self == Self::NEVER {
            return write!(f, "Never runs.");
        }
        writeln!(f, "MTWTFSS")?;
        let mut bit: usize = 2;
        while bit < 9 {
            let char = if (self.bits() & (1 << bit)) != 0 {
                'x'
            } else {
                ' '
            };
            write!(f, "{char}")?;
            bit += 1;
        }
        Ok(())
    }
}

pub fn runs_on_date(date: &Date, flags: WeekdayFlags) -> bool {
    match date.weekday() {
        Weekday::Monday if flags.contains(WeekdayFlags::MONDAY) => true,
        Weekday::Tuesday if flags.contains(WeekdayFlags::TUESDAY) => true,
        Weekday::Wednesday if flags.contains(WeekdayFlags::WEDNESDAY) => true,
        Weekday::Thursday if flags.contains(WeekdayFlags::THURSDAY) => true,
        Weekday::Friday if flags.contains(WeekdayFlags::FRIDAY) => true,
        Weekday::Saturday if flags.contains(WeekdayFlags::SATURDAY) => true,
        Weekday::Sunday if flags.contains(WeekdayFlags::SUNDAY) => true,
        _ => false,
    }
}
