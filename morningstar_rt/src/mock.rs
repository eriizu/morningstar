use super::{RealtimeStop, RealtimeStopStatus};
use jiff::{RoundMode, SignedDuration, Timestamp, TimestampRound, Unit, tz::TimeZone};
use morningstar_model::{StopTime, StopTimeWithDestination};

/// Generator to use for testing, that produces realtime and theorical data from a specific date
/// and time.
pub struct FakeGenerator {
    base_date: Timestamp,
    tz: TimeZone,
}

impl Default for FakeGenerator {
    fn default() -> Self {
        let now = Timestamp::now()
            .round(
                TimestampRound::new()
                    .smallest(Unit::Minute)
                    .mode(RoundMode::Trunc),
            )
            .expect("the current timestamp can be rounded to a minute");

        Self {
            base_date: now,
            tz: TimeZone::get("Europe/Paris").expect("Europe/Paris is in the IANA database"),
        }
    }
}

impl FakeGenerator {
    /// Generate a realtime stop time using a minute offset from the base date.
    pub fn create_realtime_stop(
        &self,
        minutes_offset: i64,
        destination: &str,
        delay_minutes: i64,
        status: RealtimeStopStatus,
    ) -> RealtimeStop {
        let aimed = self.base_date + SignedDuration::from_mins(minutes_offset);
        let expected = aimed + SignedDuration::from_mins(delay_minutes);

        RealtimeStop {
            expected_arrival: expected,
            aimed_arrival: aimed,
            destination: destination.to_string(),
            status,
        }
    }

    /// Generate a theorical stop time using a minute offset from the base date.
    pub fn create_stop_time(
        &self,
        minutes_offset: i64,
        stop_name: &str,
        stop_id: &str,
    ) -> StopTime {
        let time_with_offset = self.base_date + SignedDuration::from_mins(minutes_offset);
        let time = time_with_offset.to_zoned(self.tz.clone()).time();

        StopTime {
            time,
            stop_name: stop_name.to_string(),
            stop_id: stop_id.to_string(),
        }
    }

    /// Generate a theorical stop time using a minute offset from the base date.
    pub fn create_stop_time_with_destination(
        &self,
        minutes_offset: i64,
        stop_name: &str,
        stop_id: &str,
        destination: &str,
    ) -> StopTimeWithDestination {
        let time_with_offset = self.base_date + SignedDuration::from_mins(minutes_offset);
        let time = time_with_offset.to_zoned(self.tz.clone()).time();

        StopTimeWithDestination {
            time,
            stop_name: stop_name.to_string(),
            stop_id: stop_id.to_string(),
            destination: destination.to_string(),
            stops_to_destination: 3,
        }
    }

    /// Sample a list of fake theorical stops.
    pub fn fake_theorical_with_destination_list(&self) -> Vec<StopTimeWithDestination> {
        vec![
            self.create_stop_time_with_destination(
                -38,
                "Parc du Bel-Air",
                "IDFM:123",
                "Gare de Bussy-St-Georges",
            ),
            self.create_stop_time_with_destination(
                -8,
                "Parc du Bel-Air",
                "IDFM:123",
                "Gare de Bussy-St-Georges",
            ),
            self.create_stop_time_with_destination(
                0,
                "Parc du Bel-Air",
                "IDFM:123",
                "Gare de Bussy-St-Georges",
            ),
            self.create_stop_time_with_destination(
                10,
                "Parc du Bel-Air",
                "IDFM:123",
                "Gare de Bussy-St-Georges",
            ),
            self.create_stop_time_with_destination(
                20,
                "Parc du Bel-Air",
                "IDFM:123",
                "Gare de Bussy-St-Georges",
            ),
            self.create_stop_time_with_destination(
                40,
                "Parc du Bel-Air",
                "IDFM:123",
                "Gare de Bussy-St-Georges",
            ),
        ]
    }

    /// Sample a list of fake theorical stops.
    fn fake_theorical_list(&self) -> Vec<morningstar_model::StopTime> {
        vec![
            self.create_stop_time(-38, "Parc du Bel-Air", "IDFM:123"),
            self.create_stop_time(-8, "Parc du Bel-Air", "IDFM:123"),
            self.create_stop_time(0, "Parc du Bel-Air", "IDFM:123"),
            self.create_stop_time(10, "Parc du Bel-Air", "IDFM:123"),
            self.create_stop_time(20, "Parc du Bel-Air", "IDFM:123"),
            self.create_stop_time(40, "Parc du Bel-Air", "IDFM:123"),
        ]
    }

    /// Sample a list of fake realtime stops.
    pub fn fake_realtime_list(&self) -> Vec<RealtimeStop> {
        vec![
            self.create_realtime_stop(0, "Gare de Bussy", 2, RealtimeStopStatus::Late(2)),
            self.create_realtime_stop(10, "Gare de Bussy", -1, RealtimeStopStatus::Early(1)),
            self.create_realtime_stop(20, "Gare de Bussy", 0, RealtimeStopStatus::OnTime),
        ]
    }
}
