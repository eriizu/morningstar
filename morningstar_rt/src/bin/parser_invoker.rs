use std::str::FromStr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let invoker = morningstar_rt::parser_invoker::Invoker {
        gtfs_source: "https://www.data.gouv.fr/fr/datasets/r/f9fff5b1-f9e4-4ec2-b8b3-8ad7005d869c"
            .to_owned(),
        route_id: "IDFM:C02298".to_owned(),
        timetable_dest: std::path::PathBuf::from_str("./tt.ron").unwrap(),
    };
    let timetable = invoker.run().await?;
    dbg!(timetable.extracted_on);
    Ok(())
}
