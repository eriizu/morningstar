use super::MorningstarState;
use super::StopTimeDto;
use chrono::prelude::*;

#[poem::handler]
fn index() -> &'static str {
    "hello"
}

use poem::web::{Data, Json, Path};

#[poem::handler]
async fn served_stops(Data(state): Data<&std::sync::Arc<MorningstarState>>) -> Json<Vec<String>> {
    let today = Local::now().date_naive();
    let timetable = state.timetable.read().await;
    Json(
        timetable
            .get_stops_served_on_day(&today)
            .iter()
            .map(|val| val.to_string())
            .collect(),
    )
}

#[poem::handler]
async fn hdl_stoptimes(
    Data(state): Data<&std::sync::Arc<MorningstarState>>,
    Path(stop_name): Path<String>,
) -> Json<Vec<StopTimeDto>> {
    let stoptimes = state.next_stops_a(&stop_name).await;
    Json(stoptimes)
}

pub async fn web_server(state: std::sync::Arc<MorningstarState>) -> anyhow::Result<()> {
    use poem::{EndpointExt, Route, Server, get, listener::TcpListener};
    let routes = Route::new()
        .at("/", get(index))
        .at("/served_today", get(served_stops))
        .at("/stop/:name", get(hdl_stoptimes))
        .data(state);
    Ok(Server::new(TcpListener::bind("0.0.0.0:3000"))
        .run(routes)
        .await?)
}
