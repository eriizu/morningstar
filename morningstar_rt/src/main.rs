use morningstar_rt::web_api::{MorningstarState, timetable_update_on_expiry, web_server};

use clap::Parser;
use std::str::FromStr;

#[derive(Parser)]
struct Opt {
    #[arg(short, long)]
    file: Option<std::path::PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();
    dotenvy::dotenv()?;
    let prim_client = morningstar_rt::IdfmPrimClient::new(std::env::var("API_KEY")?);
    let (timetable, file_path) = match opt.file {
        Some(path) => {
            let file = std::fs::File::open(&path)?;
            let mut tt: morningstar_model::TimeTable = ron::de::from_reader(file)?;
            tt.sort_journeys_and_stops();
            (tt, path)
        }
        None => {
            let dest = std::path::PathBuf::from_str("./tt.ron").unwrap();
            let invoker = morningstar_rt::parser_invoker::Invoker {
                gtfs_source: "https://www.data.gouv.fr/fr/datasets/r/f9fff5b1-f9e4-4ec2-b8b3-8ad7005d869c".to_owned(),
                route_id: "IDFM:C02298".to_owned(),
                timetable_dest: dest.clone(),
            };
            let tt = invoker.run().await?;
            (tt, dest)
        }
    };
    let state = std::sync::Arc::new(MorningstarState::new(timetable, prim_client));
    let web_server_handle = tokio::spawn(web_server(state.clone()));
    let timetable_update_handle = tokio::spawn(timetable_update_on_expiry(state, file_path));
    web_server_handle.await.unwrap().unwrap();
    timetable_update_handle.await.unwrap();
    Ok(())
}
