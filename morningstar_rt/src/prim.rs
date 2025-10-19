use crate::{RealtimeStop, RealtimeStopStatus};
use chrono::{DateTime, Timelike, Utc, prelude::*};

const PRIM_STOP_ID_PREFIX: &'static str = "STIF:StopPoint:Q:";
const PRIM_STOP_ID_SUFFIX: &'static str = ":";
const GTFS_STOP_ID_PREFIX: &'static str = "IDFM:";

#[derive(Debug, PartialEq, Eq)]
pub struct StopId(String);

impl std::str::FromStr for StopId {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(
            s.trim_start_matches(PRIM_STOP_ID_PREFIX)
                .trim_start_matches(GTFS_STOP_ID_PREFIX)
                .trim_end_matches(PRIM_STOP_ID_SUFFIX)
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod test_stop_id {
    #[test]
    fn from_gtfs_id_str() {
        let stop_id: super::StopId = "IDFM:1234".parse().unwrap();
        assert_eq!(stop_id.prim(), "STIF:StopPoint:Q:1234:");
        assert_eq!(stop_id.bare(), "1234");
        assert_eq!(stop_id.gtfs().as_str(), "IDFM:1234");
    }

    #[test]
    fn from_bare_str() {
        let stop_id: super::StopId = "1234".parse().unwrap();
        assert_eq!(stop_id.prim(), "STIF:StopPoint:Q:1234:");
        assert_eq!(stop_id.bare(), "1234");
    }

    #[test]
    fn from_bare_str_with_suffix() {
        let stop_id: super::StopId = "1234:".parse().unwrap();
        assert_eq!(stop_id.prim(), "STIF:StopPoint:Q:1234:");
        assert_eq!(stop_id.bare(), "1234");
    }

    #[test]
    fn from_bare_with_prefix() {
        let stop_id: super::StopId = "STIF:StopPoint:Q:1234".parse().unwrap();
        assert_eq!(stop_id.prim(), "STIF:StopPoint:Q:1234:");
        assert_eq!(stop_id.bare(), "1234");
    }

    #[test]
    fn from_bare_with_both_affixes() {
        let stop_id: super::StopId = "STIF:StopPoint:Q:1234:".parse().unwrap();
        assert_eq!(stop_id.prim(), "STIF:StopPoint:Q:1234:");
        assert_eq!(stop_id.bare(), "1234");
    }
}

impl std::fmt::Display for StopId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'a> StopId {
    pub fn bare(&'a self) -> &'a str {
        self.0
            .trim_start_matches(PRIM_STOP_ID_PREFIX)
            .trim_end_matches(":")
    }

    pub fn gtfs(&'a self) -> String {
        let mut out = self.0.clone();
        out.insert_str(0, GTFS_STOP_ID_PREFIX);
        out
    }

    pub fn prim(&self) -> String {
        let mut out = self.0.to_string();
        out.insert_str(0, PRIM_STOP_ID_PREFIX);
        out.push_str(PRIM_STOP_ID_SUFFIX);
        out
    }
}

/// Client for https://prim.iledefrance-mobilites.fr, on which you need an account to get an
/// apikey.
pub struct IdfmPrimClient {
    api_key: String,
    api_base_url: &'static str,
    api_client: reqwest::Client,
}

impl IdfmPrimClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            api_base_url: "https://prim.iledefrance-mobilites.fr/marketplace",
            api_client: reqwest::Client::new(),
        }
    }

    pub async fn get_next_busses(&self, stop_id: &str) -> anyhow::Result<Vec<RealtimeStop>> {
        let mut url = String::from(self.api_base_url);
        url.push_str("/stop-monitoring");
        let stop_id = stop_id.parse::<StopId>()?;
        let res = self
            .api_client
            .get(url)
            .query(&[("MonitoringRef", &stop_id.prim())])
            .header("apiKey", &self.api_key)
            .send()
            .await?;

        let status = res.status();
        let body = res.text().await?;
        if status != 200 {
            return Err(anyhow::anyhow!(
                "request status {} and content {}",
                status,
                body
            ));
        }
        let root: serde_json::Value = serde_json::from_str(&body)?;

        parse_bus_info(root)
    }
}

pub fn parse_bus_info(json_value: serde_json::Value) -> anyhow::Result<Vec<RealtimeStop>> {
    let mut results = Vec::new();

    if let Some(stops) = json_value["Siri"]["ServiceDelivery"]["StopMonitoringDelivery"].as_array()
    {
        for stop in stops {
            if let Some(visits) = stop["MonitoredStopVisit"].as_array() {
                for visit in visits {
                    let journey = &visit["MonitoredVehicleJourney"];
                    let call = &journey["MonitoredCall"];

                    let destination = journey["DestinationName"]
                        .get(0)
                        .and_then(|v| v.get("value"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown")
                        .to_string();

                    let expected_arrival = call["ExpectedArrivalTime"].as_str().unwrap_or_default();
                    println!("expected {}", expected_arrival);
                    let expected_arrival = call["ExpectedArrivalTime"]
                        .as_str()
                        .unwrap_or_default()
                        .parse::<DateTime<chrono::FixedOffset>>()?;

                    let aimed_arrival = call["AimedArrivalTime"]
                        .as_str()
                        .unwrap_or_default()
                        .parse::<DateTime<chrono::FixedOffset>>()?;

                    let aimed_expected_minutes = (expected_arrival - aimed_arrival).num_minutes();

                    let status = if aimed_expected_minutes == 0 {
                        RealtimeStopStatus::OnTime
                    } else if aimed_expected_minutes > 0 {
                        RealtimeStopStatus::Late(aimed_expected_minutes)
                    } else if expected_arrival < aimed_arrival {
                        RealtimeStopStatus::Early(aimed_expected_minutes)
                    } else {
                        call["ArrivalStatus"]
                            .as_str()
                            .map(|val| RealtimeStopStatus::Other(val.to_string()))
                            .unwrap_or(RealtimeStopStatus::Unknown)
                    };

                    results.push(RealtimeStop {
                        expected_arrival,
                        aimed_arrival,
                        destination,
                        status,
                    });
                }
            }
        }
    }
    Ok(results)
}
