use crate::{IdfmPrimClient, RealtimeStop, mock};
use chrono::prelude::*;
use morningstar_model::{StopTimeWithDestination, TimeTable};

/// Makes `chrono::DateTime` from chrono::NaiveTime re-using a common timezone and basedate. We
/// need it for mass-producing absolute bus stoptimes that can be compared to the realtime date
/// returns by the IDFM-PRIM Siri-lite data.
pub struct DatetimeMaker {
    pub tz: chrono_tz::Tz,
}

impl DatetimeMaker {
    /// Create a DatetimeMaker
    pub fn new(tz_name: &str) -> Option<Self> {
        let tz = chrono_tz::TZ_VARIANTS
            .iter()
            .find(|tz| tz.name() == tz_name)?
            .clone();
        Some(Self { tz })
    }

    /// Generate a `chrono::DateTime` using the `chrono::NaiveTime` provided and the contained
    /// timezone and base date.
    pub fn make_datetime_with_time_and_tz(
        &self,
        time: NaiveTime,
    ) -> Option<chrono::DateTime<FixedOffset>> {
        use chrono::LocalResult;
        match Utc::now().with_timezone(&self.tz).with_time(time) {
            LocalResult::None => None,
            LocalResult::Single(val) => Some(val.fixed_offset()),
            LocalResult::Ambiguous(earliest, _) => Some(earliest.fixed_offset()),
        }
    }

    /// Generate a `chrono::DateTime` using the `chrono::NaiveTime` provided and the contained
    /// timezone and base date.
    pub fn datetime_with_working_tz(
        &self,
        datetime: DateTime<FixedOffset>,
    ) -> chrono::DateTime<FixedOffset> {
        self.tz
            .from_utc_datetime(&datetime.naive_utc())
            .fixed_offset()
    }
}

/// DTO for stop times, merging theorical data and realtime data when it is available.
#[derive(Debug, serde::Serialize)]
pub struct StopTimeDto {
    /// Real-time estimated call time from Siri.
    pub expected_arrival: Option<chrono::DateTime<FixedOffset>>,

    /// Theorical call time from GTFS.
    pub aimed_arrival: chrono::DateTime<FixedOffset>,

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
    fn new_with_rt_destination(
        rt: Option<&crate::RealtimeStop>,
        theorical_arrival: DateTime<FixedOffset>,
    ) -> Self {
        if let Some(rt) = rt {
            Self {
                expected_arrival: Some(rt.expected_arrival),
                aimed_arrival: rt.aimed_arrival,
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
        theorical_arrival: DateTime<FixedOffset>,
    ) -> Self {
        if let Some(rt) = rt {
            Self {
                expected_arrival: Some(rt.expected_arrival),
                aimed_arrival: rt.aimed_arrival,
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
        let aimed = self.aimed_arrival.to_utc().with_timezone(&Local);
        let expected = self
            .expected_arrival
            .map(|val| val.to_utc().with_timezone(&Local));
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

pub struct MorningstarState {
    pub timetable: RwLock<TimeTable>,
    pub prim_client: IdfmPrimClient,
    dt_maker: DatetimeMaker,
}

impl MorningstarState {
    pub fn new(timetable: TimeTable, prim_client: IdfmPrimClient) -> Self {
        println!("timetable timezone {}", timetable.timezone.as_str());
        let dt_maker = DatetimeMaker::new(timetable.timezone.as_str()).unwrap();
        Self {
            dt_maker,
            prim_client,
            timetable: RwLock::new(timetable),
        }
    }

    pub async fn next_stops_fake(&self) {
        let generator = mock::FakeGenerator::default();
        let mut stoptimes_realtime = generator.fake_realtime_list();
        let stoptimes_theorical = generator.fake_theorical_with_destination_list();
        stoptimes_realtime
            .iter_mut()
            .for_each(|item| item.really_convert_to_workting_time(&self.dt_maker));
        let dtos = self
            .mk_stoptime_dto_vec(&stoptimes_realtime, &stoptimes_theorical)
            .await;
        dtos.iter().for_each(|dto| println!("{dto}"));
    }

    pub async fn next_stops_a(&self, stop_name: &str) -> Vec<StopTimeDto> {
        let today = chrono::Local::now().naive_local().date();
        let stoptimes_theorical: Vec<_> = {
            let timetable = self.timetable.read().await;
            timetable
                .get_day_stoptimes_and_destination_for_stop(&today, stop_name)
                .collect()
        };
        let stop_id = stoptimes_theorical.last().unwrap().stop_id.as_str();
        let mut stoptimes_realtime = self.prim_client.get_next_busses(stop_id).await.unwrap();
        stoptimes_realtime
            .iter_mut()
            .for_each(|item| item.really_convert_to_workting_time(&self.dt_maker));
        let dtos = self
            .mk_stoptime_dto_vec(&stoptimes_realtime, &stoptimes_theorical)
            .await;
        dtos.iter().for_each(|dto| println!("{dto}"));
        return dtos;
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
                        .find(|realtime_stop| realtime_stop.aimed_arrival == datetime),
                    datetime,
                )
            })
            .collect::<Vec<_>>()
    }
}

pub async fn timetable_update_on_expiry(
    state: std::sync::Arc<MorningstarState>,
    file_path: std::path::PathBuf,
) {
    use chrono::Duration as ChronoDuration;
    let deadline_duration = ChronoDuration::days(7);
    loop {
        let (mut extracted_on, extracted_line_id, extracted_from) = {
            let timetable = state.timetable.read().await;
            (
                timetable.extracted_on.clone(),
                timetable.extracted_line_id.clone(),
                timetable.extracted_from.clone(),
            )
        };
        if Utc::now() >= extracted_on + deadline_duration {
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
        let delta = deadline - Utc::now();
        println!(
            "I will invoke GTFS parsing on {} in {} days {} hours {} minutes.",
            deadline,
            delta.num_days(),
            delta.num_hours() % 24,
            delta.num_minutes() % 60,
        );
        let deadline_instant = mk_instant_for_deadline(deadline);
        tokio::time::sleep_until(deadline_instant).await;
    }
}

/// Makes an monotonic Instant in order to wait for a deadline that is `duration` after `base_date`.
/// That instant can be used with `tokio::time::sleep_until` to wait for that deadline.
fn mk_deadline_instant_in_days(
    base_date: DateTime<Utc>,
    duration: chrono::Duration,
) -> tokio::time::Instant {
    use tokio::time::Duration;
    let deadline = base_date + duration;
    let now = Utc::now();
    let remaining = (deadline - now)
        .to_std()
        .unwrap_or_else(|_| Duration::from_secs(0));
    tokio::time::Instant::now() + remaining
}

fn mk_instant_for_deadline(deadline: DateTime<Utc>) -> tokio::time::Instant {
    use tokio::time::Duration;
    let now = Utc::now();
    let remaining = (deadline - now)
        .to_std()
        .unwrap_or_else(|_| Duration::from_secs(0));
    tokio::time::Instant::now() + remaining
}
