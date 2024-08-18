# Monring Star

An overly complicated way to get bus stop times, parsing the GTFS set into a custom model that can be queried using a CLI.

Rationale: my bus line is overly complicated (something like 8 different variations), PDF timetable is a mess, transport apps suck, I wanted an easy way for me to know when's the next bus to and from work. I used to write my own PDF timetables everytime new ones were published, which took a while. Then I've written one program that takes the PDF timetable, retranscribed in a CSV (painstakingly by hand because the PDFs aren't accessible), which worked OK but to a very long time to update every 6 months. Hence the "morningstar" collection of crates. Given the regional `GTFS.zip` file, they can regenerate the model my cli program can use to tell when the next buses are.

# Updating

To update the dataset, download the `IDFM_gtfs.zip` file from [IDFM's open data website](https://data.iledefrance-mobilites.fr/explore/dataset/offre-horaires-tc-gtfs-idfm/information/). Run `cargo run IDFM_gtfs.zip` in the `morningstar_parser` folder.

```sh
mv timetable.ron ../morningstar_cli
cd ../morningstar_cli
cargo buld --release
./target/release/morningstar_cli
```
