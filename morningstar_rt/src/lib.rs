pub mod mock;
mod prim;
use chrono::{DateTime, Timelike, Utc, prelude::*};
pub use prim::{IdfmPrimClient, StopId};

#[derive(Debug)]
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

#[derive(Debug)]
pub struct RealtimeStop {
    pub expected_arrival: DateTime<chrono::FixedOffset>,
    pub aimed_arrival: DateTime<chrono::FixedOffset>,
    pub destination: String,
    pub status: RealtimeStopStatus,
}

impl std::fmt::Display for RealtimeStop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let now = chrono::Utc::now();
        let delta = self.expected_arrival.to_utc() - now;
        write!(
            f,
            "{:02}:{:02} bus to {}, arives in {} mins ({})",
            self.aimed_arrival.hour(),
            self.aimed_arrival.minute(),
            self.destination,
            delta.num_minutes(),
            self.status
        )
    }
}
