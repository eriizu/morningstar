async fn print_next_busses(
    client: &morningstar_rt::IdfmPrimClient,
    id: &str,
) -> anyhow::Result<()> {
    let buses = client.get_next_busses(id).await?;

    println!("At bus stop {}:", id);
    for bus in buses {
        println!("{}", bus);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv()?;
    let client = morningstar_rt::IdfmPrimClient::new(std::env::var("API_KEY")?);
    print_next_busses(&client, "21822").await?;
    print_next_busses(&client, "21146").await?;
    print_next_busses(&client, "22487").await?;
    // Parc du bel air
    Ok(())
}
