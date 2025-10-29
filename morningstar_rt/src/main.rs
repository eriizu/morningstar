use morningstar_rt::web_api::{MorningstarState, timetable_update_on_expiry, web_server};

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
    let state = std::sync::Arc::new(MorningstarState::new(timetable, prim_client));
    state.next_stops().await;
    let web_server_handle = tokio::spawn(web_server(state.clone()));
    let file_path = opt.file.to_path_buf();
    let timetable_update_handle = tokio::spawn(timetable_update_on_expiry(state, file_path));
    web_server_handle.await.unwrap().unwrap();
    timetable_update_handle.await.unwrap();
    Ok(())
}
