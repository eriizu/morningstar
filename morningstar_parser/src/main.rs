mod extractor;

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

use clap::Parser;
#[derive(Parser)]
struct Opt {
    path_to_gtfs: String,
    line_id: String,
}

fn main() -> std::process::ExitCode {
    let opt = Opt::parse();
    let mut tt = morningstar_model::TimeTable::new();

    use spinoff::{spinners, Spinner};
    let mut spinner = Spinner::new(spinners::Dots, "Parsing", None);
    let gtfs = match gtfs_structures::Gtfs::new(&opt.path_to_gtfs) {
        Ok(gtfs) => gtfs,
        Err(err) => {
            spinner.fail(&err.to_string());
            return std::process::ExitCode::FAILURE;
        }
    };
    spinner.success("Parsing Sucessful");
    let mut spinner = Spinner::new(spinners::Dots, "Extracting to custom model", None);
    if let Err(err) = extractor::GtfsExtract::extract_gtfs_route(&mut tt, gtfs, &opt.line_id) {
        spinner.fail(&err.to_string());
        return std::process::ExitCode::FAILURE;
    }
    spinner.update_text("Serialising");
    let serialized = match ron::ser::to_string(&tt) {
        Ok(val) => val,
        Err(err) => {
            spinner.fail(&err.to_string());
            return std::process::ExitCode::FAILURE;
        }
    };
    spinner.update_text("Creating file");
    let mut file = match std::fs::File::create("timetable.ron") {
        Ok(val) => val,
        Err(err) => {
            spinner.fail(&err.to_string());
            return std::process::ExitCode::FAILURE;
        }
    };
    spinner.update_text("Writing to file");
    if let Err(err) = std::io::Write::write(&mut file, serialized.as_bytes()) {
        spinner.fail(&err.to_string());
        return std::process::ExitCode::FAILURE;
    };
    spinner.success("All done!");
    demo(tt);
    std::process::ExitCode::SUCCESS
}
