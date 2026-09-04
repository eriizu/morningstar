use jiff::{civil::Time, Zoned};

#[derive(clap::Parser)]
struct Opt {
    depart_from: Option<String>,
    // go_to: Option<String>,
    number_to_show: Option<usize>,

    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    File {
        #[arg(short, long)]
        file: std::path::PathBuf,
    },
    RealTime {
        #[arg(short, long)]
        server: String,
    },
}

impl Opt {
    async fn run(&self) {
        match self.command {
            Commands::File { ref file } => {
                self.file(file);
            }
            Commands::RealTime { ref server } => self.realtime(server).await.unwrap(),
        }
    }

    fn file(&self, file_path: &std::path::Path) {
        let tt = {
            let file = std::fs::File::open(file_path).unwrap();
            let mut tt: morningstar_model::TimeTable = ron::de::from_reader(file).unwrap();
            tt.sort_journeys_and_stops();
            tt
        };
        let (today, now) = {
            let now = Zoned::now();
            (now.date(), now.time())
        };
        if self.verbose {
            println!(
                "source file or url: {}\ncreated on: {}\nline id: {}",
                tt.extracted_from,
                tt.extracted_on.strftime("%Y-%m-%d %H:%M:%S UTC"),
                tt.extracted_line_id
            );
        }
        let stops_served_today: Vec<_> =
            tt.get_stops_served_on_day(&today).iter().copied().collect();
        if stops_served_today.is_empty() {
            eprintln!("No stops served today");
        }
        let Some(depart_from) = self.get_departure_stop(stops_served_today) else {
            eprintln!("failed to ask or match stop name");
            return;
        };
        println!("selected start stop {depart_from}");
        display_next_departures(
            tt.get_day_stoptimes_from_stop(&today, &depart_from),
            now,
            self,
        );
    }

    async fn realtime(&self, server: &str) -> Result<(), Box<dyn std::error::Error>> {
        let rt_service = morningstar_cli::rt::MorningstarRtService::new(server.to_owned());
        let served_today = rt_service.get_served_today().await?;
        if served_today.is_empty() {
            eprintln!("No stops served today");
        }
        let Some(depart_from) =
            self.get_departure_stop(served_today.iter().map(|item| item.as_str()).collect())
        else {
            eprintln!("failed to ask or match stop name");
            return Ok(());
        };
        println!("selected start stop {depart_from}");
        let resp = rt_service.get_stop(&depart_from).await?;
        use jiff::ToSpan as _;
        let now = jiff::Timestamp::now() - 0.minutes();
        resp.iter()
            .filter(|item| {
                item.expected_arrival
                    .as_ref()
                    .unwrap_or(&item.aimed_arrival)
                    .timestamp()
                    > now
            })
            .take(5)
            .for_each(|item| println!("{item}"));
        Ok(())
    }

    fn get_departure_stop(&self, stops: Vec<&str>) -> Option<String> {
        if let Some(depart_from) = &self.depart_from {
            morningstar_cli::get_best_matching_stop_name(depart_from, stops)
        } else {
            ask_for_deperture_stop(stops)
        }
    }
}

#[tokio::main]
async fn main() {
    use clap::Parser as _;
    let opt = Opt::parse();
    opt.run().await;
}

fn display_next_departures<'a, I>(iter: I, now: Time, opt: &Opt)
where
    I: Iterator<Item = &'a morningstar_model::StopTime>,
{
    iter.map(|dep| (dep.time.duration_since(now).as_mins(), dep))
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
