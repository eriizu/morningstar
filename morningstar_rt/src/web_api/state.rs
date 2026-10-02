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
        tracing::info!(timezone = %timetable.timezone, "Loading timetable");
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
        dtos.iter()
            .for_each(|dto| tracing::trace!(stop_time = %dto, "Computed stop time"));
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn next_stops_a(&self, stop_name: &str) -> Result<Vec<StopTimeDto>, StateError> {
        let today = self.today();
        let stoptimes_theorical: Vec<_> = {
            let timetable = self.timetable.read().await;
            timetable
                .get_day_stoptimes_and_destination_for_stop(&today, stop_name)
                .filter(|stoptime| stoptime.stops_to_destination > 0)
                .collect()
        };
        let stop_id = stoptimes_theorical
            .last()
            .ok_or(StateError::StopNotServed)?
            .stop_id
            .as_str();
        let stoptimes_realtime = match self.prim_client.get_next_busses(stop_id).await {
            Ok(report) => {
                for issue in &report.issues {
                    tracing::warn!(%stop_id, %issue, rejected_entry = %issue.entry, "Rejected PRIM realtime entry");
                }
                report.stops
            }
            Err(error) => {
                tracing::warn!(%stop_id, %error, "PRIM unavailable; returning scheduled times");
                Vec::new()
            }
        };
        let dtos = self
            .mk_stoptime_dto_vec(&stoptimes_realtime, &stoptimes_theorical)
            .await;
        dtos.iter()
            .for_each(|dto| tracing::trace!(stop_time = %dto, "Computed stop time"));
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
                        tracing::warn!(
                            stop_time = %stoptime_theorical.time,
                            "Stop time does not exist in destination timezone"
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
            tracing::info!(
                gtfs_source = %parser_invoker.gtfs_source,
                route_id = %parser_invoker.route_id,
                timetable_dest = %parser_invoker.timetable_dest.display(),
                "STARTING PARSING (i will eat a lot of your ram am sorry (,,>﹏<,,))"
            );
            if let Ok(val) = parser_invoker.run().await {
                extracted_on = val.extracted_on;
                *state.timetable.write().await = val;
            }
        }
        let deadline = extracted_on + deadline_duration;
        let delta = deadline.duration_since(Timestamp::now());
        tracing::info!(
            %deadline,
            remaining_seconds = delta.as_secs(),
            "I will invoke GTFS parsing on deadline"
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

    fn timetable_for_today() -> TimeTable {
        use morningstar_model::{Exception, Journey, ServiceException, StopTime};
        let mut timetable = TimeTable::new();
        let today = DatetimeMaker::new("Europe/Paris").unwrap().today();
        timetable.excpetions.insert(
            "today".into(),
            ServiceException {
                date: today,
                exception_type: Exception::Added,
            },
        );
        for hour in [12, 13] {
            timetable.journeys.push(Journey {
                service_id: "today".into(),
                stops: vec![
                    StopTime {
                        time: time(hour, 0, 0, 0),
                        stop_name: "Église".into(),
                        stop_id: "IDFM:1234".into(),
                    },
                    StopTime {
                        time: time(hour, 10, 0, 0),
                        stop_name: "Gare".into(),
                        stop_id: "IDFM:5678".into(),
                    },
                ],
            });
        }
        timetable
    }

    #[tokio::test]
    async fn returns_scheduled_times_when_prim_is_unavailable_or_rejects_all_entries() {
        use crate::prim::test_support::{envelope, mock_prim};
        use std::time::Duration;
        for (status, body, delay) in [
            (404, "missing".into(), Duration::ZERO),
            (503, "unavailable".into(), Duration::ZERO),
            (0, String::new(), Duration::ZERO),
            (200, "invalid json".into(), Duration::ZERO),
            (200, "{}".into(), Duration::ZERO),
            (200, "{}".into(), Duration::from_secs(1)),
            (
                200,
                envelope(serde_json::json!([{ "MonitoredStopVisit": [{}] }])).to_string(),
                Duration::ZERO,
            ),
        ] {
            let (client, _server) =
                mock_prim(status, body, delay, Duration::from_millis(100)).await;
            let state = MorningstarState::new(timetable_for_today(), client).unwrap();
            let dtos = state.next_stops_a("Église").await.unwrap();
            assert_eq!(dtos.len(), 2);
            for (dto, hour) in dtos.iter().zip([12, 13]) {
                assert_eq!(dto.aimed_arrival.hour(), hour);
                assert_eq!(dto.destination.as_deref(), Some("Gare"));
                assert!(dto.expected_arrival.is_none());
                assert!(dto.status.is_none());
            }
            assert!(matches!(
                state.next_stops_a("Unknown stop").await,
                Err(StateError::StopNotServed)
            ));
        }
    }

    #[tokio::test]
    async fn enriches_valid_departures_and_keeps_the_rest_scheduled() {
        use crate::prim::test_support::{envelope, mock_prim, visit};
        use std::time::Duration;
        let maker = DatetimeMaker::new("Europe/Paris").unwrap();
        let aimed = maker
            .make_datetime_with_time_and_tz(time(12, 0, 0, 0))
            .unwrap()
            .timestamp();
        let expected = aimed + SignedDuration::from_mins(5);
        let body = envelope(serde_json::json!([{ "MonitoredStopVisit": [
            visit(&aimed.to_string(), &expected.to_string()),
            visit("invalid", "invalid")
        ] }]));
        let (client, _server) = mock_prim(
            200,
            body.to_string(),
            Duration::ZERO,
            Duration::from_secs(1),
        )
        .await;
        let state = MorningstarState::new(timetable_for_today(), client).unwrap();
        let dtos = state.next_stops_a("Église").await.unwrap();
        assert_eq!(dtos.len(), 2);
        assert_eq!(
            dtos[0].expected_arrival.as_ref().unwrap().timestamp(),
            expected
        );
        assert_eq!(dtos[0].status.as_deref(), Some("late by 5'"));
        assert!(dtos[1].expected_arrival.is_none());
        assert_eq!(dtos[1].aimed_arrival.hour(), 13);
    }
}
