mod extractor;

use clap::Parser;
#[derive(Parser)]
struct Opt {
    path_to_gtfs: String,
    route_id: String,

    #[arg(short = 'o')]
    out: Option<std::path::PathBuf>,
}

fn main() -> std::process::ExitCode {
    let opt = Opt::parse();
    let mut parser = MorningstarPasrer::new();

    match parser.run_with_opt(&opt) {
        Ok(tt) => {
            demo(tt);
            std::process::ExitCode::SUCCESS
        }
        Err(err) => {
            parser.spinner.fail(&err.to_string());
            std::process::ExitCode::SUCCESS
        }
    }
}

fn if_file_get_date(fpath: &str) -> Option<chrono::DateTime<Utc>> {
    let fpath = <std::path::PathBuf as std::str::FromStr>::from_str(fpath).ok()?;
    let meta = fpath.metadata().ok()?;
    let date = chrono::DateTime::try_from(meta.created().ok()?).ok()?;
    Some(date)
}

struct MorningstarPasrer {
    spinner: spinoff::Spinner,
}

impl MorningstarPasrer {
    fn new() -> Self {
        Self {
            spinner: spinoff::Spinner::new(spinoff::spinners::Dots, "Parsing", None),
        }
    }

    fn run_with_opt(
        &mut self,
        opt: &Opt,
    ) -> Result<morningstar_model::TimeTable, Box<dyn std::error::Error>> {
        let gtfs = self.initial_parsing(&opt.path_to_gtfs)?;
        let tt = {
            let mut tt = self.extract_route(gtfs, &opt.route_id)?;
            tt.extracted_from = opt.path_to_gtfs.to_owned();
            if let Some(date) = if_file_get_date(&opt.path_to_gtfs) {
                tt.extracted_on = date;
            }
            tt.extracted_line_id = opt.route_id.clone();
            tt
        };

        self.spinner.update_text("Serialising");
        let serialized = ron::ser::to_string(&tt)?;

        self.spinner.update_text("Creating file");
        let mut file =
            std::fs::File::create(opt.out.as_ref().unwrap_or(&("timetable.ron".into())))?;

        self.spinner.update_text("Writing to file");
        std::io::Write::write(&mut file, serialized.as_bytes())?;
        self.spinner.success("All done!");

        Ok(tt)
    }

    fn initial_parsing(
        &mut self,
        path_to_gtfs: &str,
    ) -> Result<gtfs_structures::Gtfs, Box<dyn std::error::Error>> {
        let gtfs = gtfs_structures::Gtfs::new(path_to_gtfs)?;
        self.spinner.success("Parsing Sucessful");
        Ok(gtfs)
    }

    fn extract_route(
        &mut self,
        gtfs: gtfs_structures::Gtfs,
        route_id: &str,
    ) -> Result<morningstar_model::TimeTable, Box<dyn std::error::Error>> {
        self.spinner =
            spinoff::Spinner::new(spinoff::spinners::Dots, "Extracting to custom model", None);
        let mut tt = morningstar_model::TimeTable::new();
        extractor::GtfsExtract::extract_gtfs_route(&mut tt, gtfs, route_id)?;
        Ok(tt)
    }
}

use chrono::prelude::*;
fn demo(tt: morningstar_model::TimeTable) {
    let now_naive: chrono::NaiveDateTime = {
        let now = Local::now();
        now.naive_local()
    };
    let now_date = now_naive.date();
    let mut journeys: Vec<_> = tt.get_journeys_for_day(&now_date).collect();
    journeys.sort_by(|a, b| a.stops[0].time.cmp(&b.stops[0].time));
    journeys
        .iter()
        .map(|journey| (journey.stops[0].time, &journey.service_id))
        .for_each(|(time, service_id)| {
            println!("{:02}:{:02}, {}", time.hour(), time.minute(), service_id)
        });
    let tomorrow = now_date.succ_opt().unwrap();
    dbg!(tt.get_stops_served_on_day(&now_date));
    dbg!(tt.get_stops_served_on_day(&tomorrow));
}
