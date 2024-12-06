use chrono::prelude::*;
use clap::Parser;

#[derive(Parser)]
struct Opt {
    depart_from: Option<String>,
    // go_to: Option<String>,
    number_to_show: Option<usize>,

    #[arg(short, long)]
    file: std::path::PathBuf,

    #[arg(short, long)]
    verbose: bool,
}

pub fn main() {
    let opt = Opt::parse();

    let (today, now) = {
        let now = Local::now();
        let now = now.naive_local();
        (now.date(), now.time())
    };

    let tt = {
        let file = std::fs::File::open(&opt.file).unwrap();
        let mut tt: morningstar_model::TimeTable = ron::de::from_reader(file).unwrap();
        tt.sort_journeys_and_stops();
        tt
    };
    if opt.verbose {
        println!(
            "source file or url: {}\ncreated on: {}\nline id: {}",
            tt.extracted_from,
            tt.extracted_on.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            tt.extracted_line_id
        );
    }
    let stops_served_today: Vec<_> = tt.get_stops_served_on_day(&today).iter().copied().collect();
    if stops_served_today.is_empty() {
        eprintln!("No stops served today");
    }
    let Some(depart_from) = get_departure_stop(&opt, stops_served_today) else {
        eprintln!("failed to ask or match stop name");
        return;
    };
    println!("selected start stop {depart_from}");
    display_next_departures(
        tt.get_day_stoptimes_from_stop(&today, &depart_from),
        now,
        opt,
    );
}

fn display_next_departures<'a, I>(iter: I, now: NaiveTime, opt: Opt)
where
    I: Iterator<Item = &'a morningstar_model::StopTime>,
{
    iter.map(|dep| (dep.time.signed_duration_since(now).num_minutes(), dep))
        .filter(|(minutes_from_now, a)| {
            if *minutes_from_now < -10 {
                false
            } else if *minutes_from_now >= -10 && *minutes_from_now < 0 {
                print!(
                    "{:02}:{:02} (due {} minutes ago), ",
                    a.time.hour(),
                    a.time.minute(),
                    minutes_from_now * -1
                );
                false
            } else {
                true
            }
        })
        .take(opt.number_to_show.unwrap_or(3))
        .for_each(|(_, a)| {
            print!("{:02}:{:02}, ", a.time.hour(), a.time.minute(),);
        });
    println!("...");
}

fn get_departure_stop(opt: &Opt, stops: Vec<&str>) -> Option<String> {
    if let Some(depart_from) = &opt.depart_from {
        get_best_matching_stop_name(depart_from, stops)
    } else {
        ask_for_deperture_stop(stops)
    }
}

fn ask_for_deperture_stop(mut stops: Vec<&str>) -> Option<String> {
    use inquire::{error::InquireError, Select};

    stops.sort();
    let ans: Result<&str, InquireError> = Select::new("Depart from?", stops).prompt();
    match ans {
        Ok(ans) => Some(ans.to_owned()),
        Err(err) => {
            eprintln!("{err}");
            None
        }
    }
}

fn get_best_matching_stop_name(stop_name: &str, stops: Vec<&str>) -> Option<String> {
    use fuse_rust::Fuse;
    let fuse = Fuse::default();
    let results = fuse.search_text_in_iterable(stop_name, stops.iter());
    results
        .iter()
        .reduce(|acc, item| if item.score < acc.score { item } else { acc })
        .map(|best_result| stops[best_result.index].to_owned())
}
