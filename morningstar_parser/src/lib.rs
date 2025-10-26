mod extractor;
use chrono::prelude::*;

use clap::Parser;
#[derive(Parser)]
pub struct Opt {
    pub path_to_gtfs: String,
    pub route_id: String,

    #[arg(short = 'o')]
    pub out: Option<std::path::PathBuf>,
}

impl std::fmt::Display for Opt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "== Morning Star Parsing Options ==")?;
        writeln!(f, "GTFS path: {}", self.path_to_gtfs)?;
        writeln!(f, "route ID: {}", self.route_id)?;
        match &self.out {
            Some(path) => writeln!(f, "output to file: {}", path.display()),
            None => writeln!(f, "not outputing to file"),
        }
    }
}

pub fn if_file_get_date(fpath: &str) -> Option<chrono::DateTime<Utc>> {
    let fpath = <std::path::PathBuf as std::str::FromStr>::from_str(fpath).ok()?;
    let meta = fpath.metadata().ok()?;
    let date = chrono::DateTime::try_from(meta.created().ok()?).ok()?;
    Some(date)
}

pub struct MorningstarPasrer {
    pub spinner: spinoff::Spinner,
}

impl MorningstarPasrer {
    pub fn new() -> Self {
        Self {
            spinner: spinoff::Spinner::new(spinoff::spinners::Dots, "Parsing", None),
        }
    }

    pub fn run_with_opt(
        &mut self,
        opt: &Opt,
    ) -> Result<morningstar_model::TimeTable, Box<dyn std::error::Error>> {
        let gtfs = self.initial_parsing(&opt.path_to_gtfs)?;
        let tt = {
            let mut tt = self.extract_route(gtfs, &opt.route_id)?;
            tt.extracted_from = opt.path_to_gtfs.to_owned();
            if let Some(date) = if_file_get_date(&opt.path_to_gtfs) {
                tt.extracted_on = date;
            }
            tt.extracted_line_id = opt.route_id.clone();
            tt
        };

        self.spinner.update_text("Serialising");
        let serialized = ron::ser::to_string(&tt)?;

        self.spinner.update_text("Creating file");
        let mut file =
            std::fs::File::create(opt.out.as_ref().unwrap_or(&("timetable.ron".into())))?;

        self.spinner.update_text("Writing to file");
        std::io::Write::write(&mut file, serialized.as_bytes())?;
        self.spinner.success("All done!");

        Ok(tt)
    }

    fn initial_parsing(
        &mut self,
        path_to_gtfs: &str,
    ) -> Result<gtfs_structures::Gtfs, Box<dyn std::error::Error>> {
        let gtfs = gtfs_structures::Gtfs::new(path_to_gtfs)?;
        self.spinner.success("Parsing Sucessful");
        Ok(gtfs)
    }

    fn extract_route(
        &mut self,
        gtfs: gtfs_structures::Gtfs,
        route_id: &str,
    ) -> Result<morningstar_model::TimeTable, Box<dyn std::error::Error>> {
        self.spinner =
            spinoff::Spinner::new(spinoff::spinners::Dots, "Extracting to custom model", None);
        let mut tt = morningstar_model::TimeTable::new();
        extractor::GtfsExtract::extract_gtfs_route(&mut tt, gtfs, route_id)?;
        Ok(tt)
    }
}
