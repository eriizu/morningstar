use chrono::prelude::*;
use morningstar_model::{StopTime, StopTimeWithDestination, TimeTable};
use morningstar_rt::{IdfmPrimClient, RealtimeStop, RealtimeStopStatus};

/// Makes `chrono::DateTime` from chrono::NaiveTime re-using a common timezone and basedate. We
/// need it for mass-producing absolute bus stoptimes that can be compared to the realtime date
/// returns by the IDFM-PRIM Siri-lite data.
struct DatetimeMaker {
    tz: chrono_tz::Tz,
    base_date: chrono::DateTime<FixedOffset>,
}

impl DatetimeMaker {
    /// Create a DatetimeMaker
    fn new(tz_name: &str, base_date: chrono::DateTime<FixedOffset>) -> Option<Self> {
        let tz = chrono_tz::TZ_VARIANTS
            .iter()
            .find(|tz| tz.name() == tz_name)?
            .clone();
        Some(Self { tz, base_date })
    }

    /// Generate a `chrono::DateTime` using the `chrono::NaiveTime` provided and the contained
    /// timezone and base date.
    fn make_datetime_with_time_and_tz(&self, time: NaiveTime) -> chrono::DateTime<FixedOffset> {
        let date = self.tz.from_utc_datetime(&self.base_date.naive_utc());
        date.with_time(time).unwrap().fixed_offset()
    }
}

/// DTO for stop times, merging theorical data and realtime data when it is available.
#[derive(Debug, serde::Serialize)]
struct StopTimeDto {
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
        theorical: &StopTime,
        rt: Option<&morningstar_rt::RealtimeStop>,
        dt_maker: &DatetimeMaker,
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
                aimed_arrival: dt_maker.make_datetime_with_time_and_tz(theorical.time),
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
        rt: Option<&morningstar_rt::RealtimeStop>,
        dt_maker: &DatetimeMaker,
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
                aimed_arrival: dt_maker.make_datetime_with_time_and_tz(theorical.time),
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

struct MorningstarState {
    timetable: TimeTable,
    prim_client: IdfmPrimClient,
    dt_maker: DatetimeMaker,
}

impl MorningstarState {
    pub fn new(timetable: TimeTable, prim_client: IdfmPrimClient) -> Self {
        let dt_maker =
            DatetimeMaker::new(timetable.timezone.as_str(), Utc::now().fixed_offset()).unwrap();
        Self {
            dt_maker,
            prim_client,
            timetable,
        }
    }

    async fn next_stops_fake(&self) {
        let generator = FakeGenerator::default();
        let stoptimes_realtime = generator.fake_realtime_list();
        let stoptimes_theorical = generator.fake_theorical_with_destination_list();
        dbg!(&stoptimes_realtime);
        dbg!(&stoptimes_theorical);
        let dtos = self.mk_stoptime_dto_vec(&stoptimes_realtime, &stoptimes_theorical);
        dtos.iter().for_each(|dto| println!("{dto}"));
    }

    fn choose_stop(timetable: &TimeTable) -> &'static str {
        let binding = Local::now().date_naive();
        let stops_served = timetable.get_stops_served_on_day(&binding);
        dbg!(&stops_served);
        if stops_served.contains("Parc du Bel-Air") {
            return "Parc du Bel-Air";
        } else {
            return "Parc d'Activités";
        }
    }

    async fn next_stops(&self) {
        let today = chrono::Local::now().naive_local().date();
        let stop_name = Self::choose_stop(&self.timetable);
        let stoptimes_theorical: Vec<_> = self
            .timetable
            .get_day_stoptimes_and_destination_for_stop(&today, stop_name)
            .collect();
        let stop_id = stoptimes_theorical.last().unwrap().stop_id.as_str();
        let stoptimes_realtime = self.prim_client.get_next_busses(stop_id).await.unwrap();
        dbg!(&stoptimes_realtime);
        dbg!(&stoptimes_theorical);
        let dtos = self.mk_stoptime_dto_vec(&stoptimes_realtime, &stoptimes_theorical);
        dtos.iter().for_each(|dto| println!("{dto}"));
    }

    fn mk_stoptime_dto_vec(
        &self,
        stoptimes_realtime: &[RealtimeStop],
        stoptimes_theorical: &[StopTimeWithDestination],
    ) -> Vec<StopTimeDto> {
        let mut dtos = vec![];
        for stoptime in stoptimes_theorical {
            let time = self
                .dt_maker
                .make_datetime_with_time_and_tz(stoptime.time)
                .to_utc();
            let stoptime_rt_opt = stoptimes_realtime
                .iter()
                .find(|realtime_stop| realtime_stop.aimed_arrival.to_utc() == time);
            dtos.push(StopTimeDto::new_with_theorical_destination(
                stoptime,
                stoptime_rt_opt,
                &self.dt_maker,
            ));
        }
        dtos
    }

    async fn print_stoptimes(
        stoptimes_realtime: &Vec<RealtimeStop>,
        stoptimes_theorical: &Vec<&StopTimeWithDestination>,
    ) {
        let mut dtos = vec![];
        // TODO: take timezone from timetable
        let dt_maker = DatetimeMaker::new("Europe/Paris", Utc::now().fixed_offset()).unwrap();
        for stoptime in stoptimes_theorical {
            let Some(stoptime_rt) = stoptimes_realtime.iter().find(|realtime_stop| {
                realtime_stop.aimed_arrival.naive_local().time() == stoptime.time
            }) else {
                dtos.push(StopTimeDto::new_with_theorical_destination(
                    stoptime, None, &dt_maker,
                ));
                println!("{:02}:{:02}", stoptime.time.hour(), stoptime.time.minute());
                continue;
            };
            dtos.push(StopTimeDto::new_with_theorical_destination(
                stoptime,
                Some(stoptime_rt),
                &dt_maker,
            ));
            println!("{}", stoptime_rt);
        }
        serde_json::ser::to_writer_pretty(std::io::stdout(), &dtos).unwrap();
    }
}

use clap::Parser;

#[derive(Parser)]
struct Opt {
    #[arg(short, long)]
    file: std::path::PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();
    dotenvy::dotenv()?;
    let prim_client = morningstar_rt::IdfmPrimClient::new(std::env::var("API_KEY")?);
    let timetable = {
        let file = std::fs::File::open(&opt.file).unwrap();
        let mut tt: morningstar_model::TimeTable = ron::de::from_reader(file).unwrap();
        tt.sort_journeys_and_stops();
        tt
    };
    let state = MorningstarState::new(timetable, prim_client);
    state.next_stops().await;
    Ok(())
}

/// Generator to use for testing, that produces realtime and theorical data from a specific date
/// and time.
struct FakeGenerator {
    base_date: chrono::DateTime<FixedOffset>,
}

impl Default for FakeGenerator {
    fn default() -> Self {
        let tz = chrono_tz::Europe::Paris;
        let now = tz
            .from_utc_datetime(
                &Utc::now()
                    .with_second(0)
                    .unwrap()
                    .with_nanosecond(0)
                    .unwrap()
                    .naive_utc(),
            )
            .fixed_offset();

        Self { base_date: now }
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
        let aimed = self.base_date + chrono::Duration::minutes(minutes_offset);
        let expected = aimed + chrono::Duration::minutes(delay_minutes);

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
        let time_with_offset = self.base_date + chrono::Duration::minutes(minutes_offset);

        StopTime {
            time: time_with_offset.time(),
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
        let time_with_offset = self.base_date + chrono::Duration::minutes(minutes_offset);

        StopTimeWithDestination {
            time: time_with_offset.time(),
            stop_name: stop_name.to_string(),
            stop_id: stop_id.to_string(),
            destination: destination.to_string(),
            stops_to_destination: 3,
        }
    }

    /// Sample a list of fake theorical stops.
    fn fake_theorical_with_destination_list(&self) -> Vec<StopTimeWithDestination> {
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
