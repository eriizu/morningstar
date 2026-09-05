use crate::{IdfmPrimClient, RealtimeStop, mock};
use jiff::{
    SignedDuration, Timestamp, Zoned,
    civil::Time,
    tz::{AmbiguousOffset, TimeZone},
};
use morningstar_model::{StopTimeWithDestination, TimeTable};

/// Makes a `Zoned` datetime from a civil `Time`, reusing a common timezone and base date. We
/// need it for mass-producing absolute bus stoptimes that can be compared to the realtime date
/// returns by the IDFM-PRIM Siri-lite data.
pub struct DatetimeMaker {
    pub tz: TimeZone,
}

impl DatetimeMaker {
    /// Create a DatetimeMaker
    pub fn new(tz_name: &str) -> Result<Self, StateError> {
        let tz = TimeZone::get(tz_name).map_err(|err| StateError::TimezoneNonExistent(err))?;
        Ok(Self { tz })
    }

    /// Generate a timestamp using the provided civil time and the current date in this timezone.
    /// Nonexistent times in a DST gap are rejected; the earlier instant is used for a DST fold.
    pub fn make_datetime_with_time_and_tz(&self, time: Time) -> Option<Zoned> {
        self.make_datetime_on_date(self.today(), time)
    }

    fn make_datetime_on_date(&self, date: jiff::civil::Date, time: Time) -> Option<Zoned> {
        let datetime = date.to_datetime(time);
        let ambiguous = self.tz.to_ambiguous_zoned(datetime);
        match ambiguous.offset() {
            AmbiguousOffset::Gap { .. } => None,
            AmbiguousOffset::Fold { .. } => ambiguous.earlier().ok(),
            AmbiguousOffset::Unambiguous { .. } => ambiguous.unambiguous().ok(),
        }
    }

    pub fn today(&self) -> jiff::civil::Date {
        Timestamp::now().to_zoned(self.tz.clone()).date()
    }
}

/// DTO for stop times, merging theorical data and realtime data when it is available.
#[derive(Debug, serde::Serialize)]
pub struct StopTimeDto {
    /// Real-time estimated call time from Siri.
    // #[serde(serialize_with = "serialize_optional_zoned_as_offset_datetime")]
    pub expected_arrival: Option<Zoned>,

    /// Theorical call time from GTFS.
    // #[serde(serialize_with = "serialize_zoned_as_offset_datetime")]
    pub aimed_arrival: Zoned,

    /// Destination (usually generated from Siri)
    pub destination: Option<String>,

    /// Number of stops between this stop and destination.
    pub stops_to_destination: Option<u32>,

    /// Real-time status from Siri.
    pub status: Option<String>,
}

impl StopTimeDto {
    /// Make a `StopTimeDto` from theorical and realtime data (when avail.) using a `DatetimeMaker`
    /// for absolute call datetimes.
    fn new_with_rt_destination(rt: Option<&crate::RealtimeStop>, theorical_arrival: Zoned) -> Self {
        if let Some(rt) = rt {
            let tz = theorical_arrival.time_zone().clone();
            Self {
                expected_arrival: Some(rt.expected_arrival.to_zoned(tz.clone())),
                aimed_arrival: rt.aimed_arrival.to_zoned(tz),
                destination: Some(rt.destination.clone()),
                status: Some(rt.status.to_string()),
                stops_to_destination: None,
            }
        } else {
            Self {
                expected_arrival: None,
                aimed_arrival: theorical_arrival,
                destination: None,
                status: None,
                stops_to_destination: None,
            }
        }
    }

    /// Make a `StopTimeDto` from theorical and realtime data (when avail.) using a `DatetimeMaker`
    /// for absolute call datetimes.
    fn new_with_theorical_destination(
        theorical: &StopTimeWithDestination,
        rt: Option<&crate::RealtimeStop>,
        theorical_arrival: Zoned,
    ) -> Self {
        if let Some(rt) = rt {
            let tz = theorical_arrival.time_zone().clone();
            Self {
                expected_arrival: Some(rt.expected_arrival.to_zoned(tz.clone())),
                aimed_arrival: rt.aimed_arrival.to_zoned(tz),
                destination: Some(theorical.destination.clone()),
                status: Some(rt.status.to_string()),
                stops_to_destination: Some(theorical.stops_to_destination),
            }
        } else {
            Self {
                expected_arrival: None,
                aimed_arrival: theorical_arrival,
                destination: Some(theorical.destination.clone()),
                status: None,
                stops_to_destination: Some(theorical.stops_to_destination),
            }
        }
    }
}

impl std::fmt::Display for StopTimeDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let local_tz = TimeZone::system();
        let aimed = self.aimed_arrival.timestamp().to_zoned(local_tz.clone());
        let expected = self
            .expected_arrival
            .as_ref()
            .map(|val| val.timestamp().to_zoned(local_tz));
        write!(f, "{:02}:{:02}", aimed.hour(), aimed.minute())?;
        if let Some(destination) = &self.destination {
            write!(f, " to {}", destination)?;
        }
        if let Some(stops) = &self.stops_to_destination {
            write!(f, " in {} stops", stops)?;
        }
        if let Some(expected_arrival) = expected {
            write!(
                f,
                " expected {:02}:{:02}",
                expected_arrival.hour(),
                expected_arrival.minute()
            )?;
        }
        if let Some(status) = &self.status {
            write!(f, " ({})", status)?;
        }
        Ok(())
    }
}

use tokio::sync::RwLock;

#[derive(thiserror::Error, Debug)]
pub enum StateError {
    #[error("stop not served or does not exist")]
    StopNotServed,
    #[error("querying the PRIM for realtime data: {_0}")]
    Prim(anyhow::Error),
    #[error("timezone does not exist: {_0}")]
    TimezoneNonExistent(jiff::Error),
}

pub struct MorningstarState {
    pub timetable: RwLock<TimeTable>,
    pub prim_client: IdfmPrimClient,
    dt_maker: DatetimeMaker,
}

impl MorningstarState {
    pub fn new(timetable: TimeTable, prim_client: IdfmPrimClient) -> Result<Self, StateError> {
        println!("timetable timezone {}", timetable.timezone.as_str());
        let dt_maker = DatetimeMaker::new(timetable.timezone.as_str())?;
        Ok(Self {
            dt_maker,
            prim_client,
            timetable: RwLock::new(timetable),
        })
    }

    pub fn today(&self) -> jiff::civil::Date {
        self.dt_maker.today()
    }

    pub async fn next_stops_fake(&self) {
        let generator = mock::FakeGenerator::default();
        let stoptimes_realtime = generator.fake_realtime_list();
        let stoptimes_theorical = generator.fake_theorical_with_destination_list();
        let dtos = self
            .mk_stoptime_dto_vec(&stoptimes_realtime, &stoptimes_theorical)
            .await;
        dtos.iter().for_each(|dto| println!("{dto}"));
    }

    pub async fn next_stops_a(&self, stop_name: &str) -> Result<Vec<StopTimeDto>, StateError> {
        let today = self.today();
        let stoptimes_theorical: Vec<_> = {
            let timetable = self.timetable.read().await;
            timetable
                .get_day_stoptimes_and_destination_for_stop(&today, stop_name)
                .collect()
        };
        let stop_id = stoptimes_theorical
            .last()
            .ok_or(StateError::StopNotServed)?
            .stop_id
            .as_str();
        let stoptimes_realtime = self
            .prim_client
            .get_next_busses(stop_id)
            .await
            .map_err(|err| StateError::Prim(err))?;
        let dtos = self
            .mk_stoptime_dto_vec(&stoptimes_realtime, &stoptimes_theorical)
            .await;
        dtos.iter().for_each(|dto| println!("{dto}"));
        Ok(dtos)
    }

    async fn mk_stoptime_dto_vec(
        &self,
        stoptimes_realtime: &[RealtimeStop],
        stoptimes_theorical: &[StopTimeWithDestination],
    ) -> Vec<StopTimeDto> {
        stoptimes_theorical
            .iter()
            .filter_map(|stoptime_theorical| {
                self.dt_maker
                    .make_datetime_with_time_and_tz(stoptime_theorical.time)
                    .map(|datetime| (stoptime_theorical, datetime))
                    .or_else(|| {
                        eprintln!(
                            "stop time {} doesn't exist in destination timezone.",
                            stoptime_theorical.time
                        );
                        None
                    })
            })
            .map(|(stoptime_theorical, datetime)| {
                StopTimeDto::new_with_theorical_destination(
                    stoptime_theorical,
                    stoptimes_realtime
                        .iter()
                        .find(|realtime_stop| realtime_stop.aimed_arrival == datetime.timestamp()),
                    datetime,
                )
            })
            .collect::<Vec<_>>()
    }
}

fn serialize_zoned_as_offset_datetime<S>(value: &Zoned, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.collect_str(&value.timestamp().display_with_offset(value.offset()))
}

fn serialize_optional_zoned_as_offset_datetime<S>(
    value: &Option<Zoned>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match value {
        Some(value) => serializer.serialize_some(
            &value
                .timestamp()
                .display_with_offset(value.offset())
                .to_string(),
        ),
        None => serializer.serialize_none(),
    }
}

pub async fn timetable_update_on_expiry(
    state: std::sync::Arc<MorningstarState>,
    file_path: std::path::PathBuf,
) {
    let deadline_duration = SignedDuration::from_hours(7 * 24);
    loop {
        let (mut extracted_on, extracted_line_id, extracted_from) = {
            let timetable = state.timetable.read().await;
            (
                timetable.extracted_on,
                timetable.extracted_line_id.clone(),
                timetable.extracted_from.clone(),
            )
        };
        if Timestamp::now() >= extracted_on + deadline_duration {
            let parser_invoker = crate::parser_invoker::Invoker {
                gtfs_source: extracted_from,
                route_id: extracted_line_id,
                timetable_dest: file_path.to_path_buf(),
            };
            println!("STARTING PARSING (i will eat a lot of your ram am sorry (,,>﹏<,,))");
            println!("{}", parser_invoker);
            if let Ok(val) = parser_invoker.run().await {
                extracted_on = val.extracted_on;
                *state.timetable.write().await = val;
            }
        }
        let deadline = extracted_on + deadline_duration;
        let delta = deadline.duration_since(Timestamp::now());
        println!(
            "I will invoke GTFS parsing on {} in {} days {} hours {} minutes.",
            deadline,
            delta.as_hours() / 24,
            delta.as_hours() % 24,
            delta.as_mins() % 60,
        );
        let deadline_instant = mk_instant_for_deadline(deadline);
        tokio::time::sleep_until(deadline_instant).await;
    }
}

/// Makes an monotonic Instant in order to wait for a deadline that is `duration` after `base_date`.
/// That instant can be used with `tokio::time::sleep_until` to wait for that deadline.
fn mk_deadline_instant_in_days(
    base_date: Timestamp,
    duration: SignedDuration,
) -> tokio::time::Instant {
    use tokio::time::Duration;
    let deadline = base_date + duration;
    let now = Timestamp::now();
    let remaining =
        Duration::try_from(deadline.duration_since(now)).unwrap_or_else(|_| Duration::from_secs(0));
    tokio::time::Instant::now() + remaining
}

fn mk_instant_for_deadline(deadline: Timestamp) -> tokio::time::Instant {
    use tokio::time::Duration;
    let now = Timestamp::now();
    let remaining =
        Duration::try_from(deadline.duration_since(now)).unwrap_or_else(|_| Duration::from_secs(0));
    tokio::time::Instant::now() + remaining
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::{date, time};

    #[test]
    fn rejects_a_time_in_a_dst_gap() {
        let maker = DatetimeMaker::new("Europe/Paris").unwrap();

        assert!(
            maker
                .make_datetime_on_date(date(2024, 3, 31), time(2, 30, 0, 0))
                .is_none()
        );
    }

    #[test]
    fn selects_the_earlier_instant_in_a_dst_fold() {
        let maker = DatetimeMaker::new("Europe/Paris").unwrap();

        let timestamp = maker
            .make_datetime_on_date(date(2024, 10, 27), time(2, 30, 0, 0))
            .unwrap()
            .timestamp();

        assert_eq!(timestamp, "2024-10-27T00:30:00Z".parse().unwrap());
    }

    #[test]
    fn dto_json_keeps_an_offset_datetime_without_a_zone_annotation() {
        let aimed_arrival = "2024-10-27T00:30:00Z"
            .parse::<Timestamp>()
            .unwrap()
            .in_tz("Europe/Paris")
            .unwrap();
        let dto = StopTimeDto {
            expected_arrival: None,
            aimed_arrival,
            destination: None,
            stops_to_destination: None,
            status: None,
        };

        let json = serde_json::to_value(dto).unwrap();

        assert_eq!(json["aimed_arrival"], "2024-10-27T02:30:00+02:00");
        assert!(json["expected_arrival"].is_null());
    }
}
