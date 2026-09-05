use super::MorningstarState;
use super::StopTimeDto;
use poem::web::{Data, Html, Json, Path};

#[poem::handler]
fn index() -> Html<&'static str> {
    Html(include_str!("../../../morningstar_fe/index.html"))
}

#[poem::handler]
async fn served_stops(Data(state): Data<&std::sync::Arc<MorningstarState>>) -> Json<Vec<String>> {
    let today = state.today();
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
) -> Result<Json<Vec<StopTimeDto>>, poem::http::StatusCode> {
    match state.next_stops_a(&stop_name).await {
        Ok(stoptimes) => Ok(Json(stoptimes)),
        Err(super::StateError::StopNotServed) => Err(poem::http::StatusCode::NOT_FOUND),
        Err(err) => {
            eprintln!("{err}");
            Err(poem::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn web_server(state: std::sync::Arc<MorningstarState>) -> anyhow::Result<()> {
    use poem::{EndpointExt, Route, Server, get, listener::TcpListener, middleware::Cors};
    let cors = Cors::new();
    let routes = Route::new()
        .at("/", get(index))
        .at("/served_today", get(served_stops))
        .at("/stop/:name", get(hdl_stoptimes))
        .with(cors)
        .data(state);
    Ok(Server::new(TcpListener::bind("0.0.0.0:3000"))
        .run(routes)
        .await?)
}
