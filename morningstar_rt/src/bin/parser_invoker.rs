use std::str::FromStr;

use anyhow::Context as _;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let invoker = morningstar_rt::parser_invoker::Invoker {
        gtfs_source: "https://www.data.gouv.fr/fr/datasets/r/f9fff5b1-f9e4-4ec2-b8b3-8ad7005d869c"
            .to_owned(),
        route_id: "IDFM:C02298".to_owned(),
        timetable_dest: std::path::PathBuf::from_str("./tt.ron").context("building the invoker")?,
    };
    let timetable = invoker.run().await?;
    tracing::info!(extracted_on = %timetable.extracted_on, "Timetable loaded");
    Ok(())
}
