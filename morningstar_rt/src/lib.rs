pub mod mock;
mod prim;
use jiff::{Timestamp, tz::TimeZone};
pub use prim::{IdfmPrimClient, StopId};
pub mod parser_invoker;
pub mod web_api;

#[derive(Debug, Clone)]
pub enum RealtimeStopStatus {
    Early(i64),
    OnTime,
    Late(i64),
    Other(String),
    Unknown,
}

impl std::fmt::Display for RealtimeStopStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Early(minutes) => write!(f, "early by {}'", minutes),
            Self::OnTime => write!(f, "on time"),
            Self::Late(minutes) => write!(f, "late by {}'", minutes),
            Self::Other(val) => write!(f, "{}", val),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RealtimeStop {
    pub expected_arrival: Timestamp,
    pub aimed_arrival: Timestamp,
    pub destination: String,
    pub status: RealtimeStopStatus,
}

impl std::fmt::Display for RealtimeStop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let now = Timestamp::now();
        let delta = self.expected_arrival.duration_since(now);
        let aimed = self.aimed_arrival.to_zoned(TimeZone::system());
        write!(
            f,
            "{:02}:{:02} bus to {}, arives in {} mins ({})",
            aimed.hour(),
            aimed.minute(),
            self.destination,
            delta.as_mins(),
            self.status
        )
    }
}
