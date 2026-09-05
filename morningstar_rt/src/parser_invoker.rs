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
        tracing::info!("GTFS parsing and new timetable ingestion complete");
        Ok(timetable)
    }

    async fn spawn(&self, parser_path: &str) -> InvokerResult<tokio::process::Child> {
        use std::process::Stdio;
        use tokio::process::Command;

        tracing::debug!(parser_path, "Spawning parser process");
        let child = Command::new(parser_path)
            .arg(self.gtfs_source.clone())
            .arg(self.route_id.clone())
            .arg("-o")
            .arg(self.timetable_dest.clone())
            .kill_on_drop(true)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| Error::ProcessSpawn(err))?;
        tracing::debug!(pid = child.id(), "Parser process spawned");
        Ok(child)
    }

    async fn await_child(mut child: tokio::process::Child) -> InvokerResult<()> {
        tracing::debug!("Waiting for parser process");
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (status, (), ()) = tokio::try_join!(
            child.wait(),
            Self::trace_output(stdout, false),
            Self::trace_output(stderr, true),
        )
        .map_err(Error::ProcessWait)?;
        tracing::debug!(%status, "Parser process exited");
        if !status.success() {
            Err(Error::ParserError)
        } else {
            tracing::info!("GTFS parsed");
            Ok(())
        }
    }

    // Drain both pipes concurrently with the child so verbose output cannot block parsing.
    async fn trace_output(
        output: Option<impl tokio::io::AsyncRead + Unpin>,
        is_stderr: bool,
    ) -> std::io::Result<()> {
        use tokio::io::{AsyncBufReadExt, BufReader};

        if let Some(output) = output {
            let mut lines = BufReader::new(output).lines();
            while let Some(line) = lines.next_line().await? {
                if is_stderr {
                    tracing::warn!(target: "morningstar_rt::parser_invoker", %line, "Parser stderr");
                } else {
                    tracing::debug!(target: "morningstar_rt::parser_invoker", %line, "Parser stdout");
                }
            }
        }
        Ok(())
    }

    async fn ingest_file(
        file_path: std::path::PathBuf,
    ) -> InvokerResult<morningstar_model::TimeTable> {
        tracing::debug!(path = %file_path.display(), "Spawning timetable deserialisation task");
        let task = tokio::task::spawn_blocking(move || Self::ingest_file_sync(file_path));
        task.await.map_err(|err| Error::FileProcessingTask(err))?
    }

    fn ingest_file_sync(
        file_path: std::path::PathBuf,
    ) -> InvokerResult<morningstar_model::TimeTable> {
        tracing::debug!(path = %file_path.display(), "Opening and deserialising timetable");
        let file = std::fs::File::open(file_path).map_err(|err| Error::FileOpening(err))?;
        Ok(ron::de::from_reader(file).map_err(|err| Error::FileProcessing(err))?)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::process::Stdio;

    #[tokio::test]
    async fn drains_verbose_parser_output_without_blocking() {
        let child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg("i=0; while [ $i -lt 10000 ]; do echo parser-output; echo parser-warning >&2; i=$((i + 1)); done")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();

        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            Invoker::await_child(child),
        )
        .await
        .expect("parser output should be drained even without a tracing subscriber")
        .unwrap();
    }

    #[tokio::test]
    async fn reports_parser_failure_after_draining_output() {
        let child = tokio::process::Command::new("sh")
            .args(["-c", "echo parser-failed >&2; exit 1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();

        assert!(matches!(
            Invoker::await_child(child).await,
            Err(Error::ParserError)
        ));
    }
}
