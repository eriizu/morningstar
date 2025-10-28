#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("spawning parser: {_0}")]
    ProcessSpawn(tokio::io::Error),
    #[error("waiting on parser process: {_0}")]
    ProcessWait(tokio::io::Error),
    #[error("parser failed, no details available")]
    ParserError,
    #[error("failed opening timetable file: {_0}")]
    FileOpening(std::io::Error),
    #[error("failed ingesting timetable file: {_0}")]
    FileProcessing(ron::de::SpannedError),
    #[error("failed to join on the file processing task: {_0}")]
    FileProcessingTask(tokio::task::JoinError),
    #[error("opt must contain a filepath")]
    MissingFilePath,
}

pub type InvokerResult<T> = Result<T, Error>;

pub struct Invoker {
    pub gtfs_source: String,
    pub route_id: String,
    pub timetable_dest: std::path::PathBuf,
}

impl std::fmt::Display for Invoker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "== Parser Invoker Options ==")?;
        writeln!(f, "GTFS source: {}", self.gtfs_source)?;
        writeln!(f, "route ID: {}", self.route_id)?;
        writeln!(
            f,
            "parsed timetable destination: {}",
            self.timetable_dest.display()
        )?;
        Ok(())
    }
}

impl Invoker {
    pub async fn run(&self) -> InvokerResult<morningstar_model::TimeTable> {
        let child_process = self.spawn("morningstar_parser").await?;
        Self::await_child(child_process).await?;
        let timetable = Self::ingest_file(self.timetable_dest.clone()).await?;
        println!("GTFS parsing and new timetable ingestion complete");
        Ok(timetable)
    }

    async fn spawn(&self, parser_path: &str) -> InvokerResult<tokio::process::Child> {
        use std::process::Stdio;
        use tokio::process::Command;

        println!("spawning process");
        let child = Command::new(parser_path)
            .arg(self.gtfs_source.clone())
            .arg(self.route_id.clone())
            .arg("-o")
            .arg(self.timetable_dest.clone())
            .kill_on_drop(true)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|err| Error::ProcessSpawn(err))?;
        println!("process spawned");
        Ok(child)
    }

    async fn await_child(mut child: tokio::process::Child) -> InvokerResult<()> {
        println!("awaiting child");
        let status = child.wait().await.map_err(|err| Error::ProcessWait(err))?;
        println!("child exited");
        if !status.success() {
            Err(Error::ParserError)
        } else {
            println!("GTFS parsed");
            Ok(())
        }
    }

    async fn ingest_file(
        file_path: std::path::PathBuf,
    ) -> InvokerResult<morningstar_model::TimeTable> {
        println!("spawning task to deserialise new timetable");
        let task = tokio::task::spawn_blocking(move || Self::ingest_file_sync(file_path));
        task.await.map_err(|err| Error::FileProcessingTask(err))?
    }

    fn ingest_file_sync(
        file_path: std::path::PathBuf,
    ) -> InvokerResult<morningstar_model::TimeTable> {
        println!("opening file and deserialising");
        let file = std::fs::File::open(file_path).map_err(|err| Error::FileOpening(err))?;
        Ok(ron::de::from_reader(file).map_err(|err| Error::FileProcessing(err))?)
    }
}
