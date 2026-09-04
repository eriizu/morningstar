#[derive(Debug, serde::Deserialize)]
pub struct StopTimeDto {
    /// Real-time estimated call time from Siri.
    pub expected_arrival: Option<jiff::Zoned>,

    /// Theorical call time from GTFS.
    pub aimed_arrival: jiff::Zoned,

    /// Destination (usually generated from Siri)
    pub destination: Option<String>,

    /// Number of stops between this stop and destination.
    pub stops_to_destination: Option<u32>,

    /// Real-time status from Siri.
    pub status: Option<String>,
}

impl std::fmt::Display for StopTimeDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let time_and_details = self
            .expected_arrival
            .as_ref()
            .map(|val| {
                let time = val.strftime("%H:%M:%S").to_string();
                // let delta = jiff::Timestamp::now().until(val).unwrap().get_minutes();
                let delta = jiff::Timestamp::now()
                    .to_zoned(val.time_zone().clone())
                    .until(val)
                    .unwrap();
                let delta_secs = delta.get_seconds();
                let delta_mins = delta.get_minutes();
                self.status
                    .as_ref()
                    .map(|status| format!("{delta_mins:02}m {delta_secs:02}s -- {time} ({status})"))
                    .unwrap_or(time)
            })
            .unwrap_or_else(|| {
                self.aimed_arrival
                    .strftime("%H:%M:%S (theorical)")
                    .to_string()
            });
        write!(f, "{}", time_and_details)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum RtServiceError {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error("stop does not exist")]
    StopNotFound,
}

pub struct MorningstarRtService {
    base_url: String,
}

impl MorningstarRtService {
    pub fn new(mut base_url: String) -> Self {
        if base_url.ends_with("/") {
            base_url.pop();
        }
        Self { base_url }
    }

    pub async fn get_served_today(&self) -> Result<Vec<String>, RtServiceError> {
        let resp = reqwest::get(format!("{}/served_today", self.base_url)).await?;
        let served_today = resp.json::<Vec<String>>().await?;
        Ok(served_today)
    }

    pub async fn get_stop(&self, stop_name: &str) -> Result<Vec<StopTimeDto>, RtServiceError> {
        let stop_times = match reqwest::get(format!("{}/stop/{}", self.base_url, stop_name))
            .await?
            .json::<Vec<StopTimeDto>>()
            .await
        {
            Ok(val) => Ok(val),
            Err(err) if err.is_status() => {
                if 404 == err.status().expect("status to be set if is_status is true") {
                    Err(RtServiceError::StopNotFound)
                } else {
                    Err(RtServiceError::Reqwest(err))
                }
            }
            Err(err) => Err(RtServiceError::Reqwest(err)),
        }?;
        Ok(stop_times)
    }
}
