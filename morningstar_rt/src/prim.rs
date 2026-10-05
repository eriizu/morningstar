use crate::{RealtimeStop, RealtimeStopStatus};
use jiff::Timestamp;

const PRIM_STOP_ID_PREFIX: &str = "STIF:StopPoint:Q:";
const PRIM_STOP_ID_SUFFIX: &str = ":";
const GTFS_STOP_ID_PREFIX: &str = "IDFM:";

/// How long a cached stop-monitoring response stays valid.
const CACHE_TTL: jiff::SignedDuration = jiff::SignedDuration::from_secs(20);
/// Shorter validity used while a bus is imminent, so the countdown stays accurate.
const IMMINENT_CACHE_TTL: jiff::SignedDuration = jiff::SignedDuration::from_secs(2);
/// A bus expected within this delay (or already past) is considered imminent.
const IMMINENT_THRESHOLD: jiff::SignedDuration = jiff::SignedDuration::from_mins(1);

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct StopId(String);

impl std::str::FromStr for StopId {
    type Err = std::convert::Infallible;
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

    #[test]
    fn parses_realtime_offset_timestamps_as_instants() {
        let json = serde_json::json!({
            "Siri": {
                "ServiceDelivery": {
                    "StopMonitoringDelivery": [{
                        "MonitoredStopVisit": [{
                            "MonitoredVehicleJourney": {
                                "DestinationName": [{ "value": "Gare" }],
                                "MonitoredCall": {
                                    "ExpectedArrivalTime": "2024-10-27T02:35:00+02:00",
                                    "AimedArrivalTime": "2024-10-27T02:30:00+02:00"
                                }
                            }
                        }]
                    }]
                }
            }
        });

        let stops = super::parse_bus_info(json).unwrap().stops;

        assert_eq!(stops.len(), 1);
        assert_eq!(stops[0].aimed_arrival.to_string(), "2024-10-27T00:30:00Z");
        assert_eq!(
            stops[0].expected_arrival.to_string(),
            "2024-10-27T00:35:00Z"
        );
        assert_eq!(stops[0].status.to_string(), "late by 5'");
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

/// A failure affecting the whole realtime lookup.
#[derive(Debug, thiserror::Error)]
pub enum PrimError {
    #[error("PRIM transport failure: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("PRIM returned HTTP {status}")]
    Http { status: reqwest::StatusCode },
    #[error("PRIM returned invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("invalid PRIM response envelope: {field}")]
    InvalidEnvelope { field: &'static str },
    #[error("PRIM reported an unsuccessful service delivery")]
    ServiceDeliveryFailed,
    #[error("PRIM cache lock is poisoned")]
    CacheUnavailable,
}

/// A rejected delivery or visit, with its position and offending field.
#[derive(Debug, Clone, thiserror::Error)]
#[error("delivery {delivery_index}, visit {visit_index:?}, field {field}: {reason}")]
pub struct ParseIssue {
    pub delivery_index: usize,
    pub visit_index: Option<usize>,
    pub field: &'static str,
    pub reason: String,
    /// Original JSON for the rejected visit, or the entire rejected delivery.
    pub entry: serde_json::Value,
}

/// Usable predictions and diagnostics for entries that were rejected.
#[derive(Debug, Clone, Default)]
pub struct RealtimeReport {
    pub stops: Vec<RealtimeStop>,
    pub issues: Vec<ParseIssue>,
}

impl RealtimeReport {
    /// Cache validity, shortened when any bus is expected less than a minute from `now`.
    fn cache_ttl(&self, now: Timestamp) -> jiff::SignedDuration {
        let imminent = self
            .stops
            .iter()
            .any(|stop| stop.expected_arrival.duration_since(now) < IMMINENT_THRESHOLD);
        if imminent { IMMINENT_CACHE_TTL } else { CACHE_TTL }
    }
}

/// Client for https://prim.iledefrance-mobilites.fr, on which you need an account to get an
/// apikey.
pub struct IdfmPrimClient {
    api_key: String,
    api_base_url: String,
    api_client: reqwest::Client,
    request_timeout: std::time::Duration,
    stop_monitoring_cache:
        std::sync::RwLock<std::collections::HashMap<StopId, (Timestamp, RealtimeReport)>>,
}

impl IdfmPrimClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            api_base_url: "https://prim.iledefrance-mobilites.fr/marketplace".into(),
            api_client: reqwest::Client::new(),
            request_timeout: std::time::Duration::from_secs(5),
            stop_monitoring_cache: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn get_next_busses(&self, stop_id: &str) -> Result<RealtimeReport, PrimError> {
        let stop_id = stop_id
            .parse::<StopId>()
            .expect("StopId parsing is infallible");
        let cached = self
            .stop_monitoring_cache
            .read()
            .map_err(|_| PrimError::CacheUnavailable)?
            .get(&stop_id)
            .and_then(|(date, report)| {
                let now = Timestamp::now();
                let delta = now.duration_since(*date);
                if delta <= report.cache_ttl(now) {
                    tracing::debug!(cache_age_seconds = delta.as_secs(), %stop_id,
                        "Using cached realtime stops");
                    Some(report.clone())
                } else {
                    None
                }
            });
        if let Some(cached) = cached {
            return Ok(cached);
        }
        let response_body = self.api_rq_stop_monitoring(&stop_id).await?;
        let report = parse_bus_info(response_body)?;
        // An entirely rejected response must not become a cached successful empty lookup.
        if !report.stops.is_empty() || report.issues.is_empty() {
            self.stop_monitoring_cache
                .write()
                .map_err(|_| PrimError::CacheUnavailable)?
                .insert(stop_id, (Timestamp::now(), report.clone()));
        }
        Ok(report)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn api_rq_stop_monitoring(
        &self,
        stop_id: &StopId,
    ) -> Result<serde_json::Value, PrimError> {
        let res = self
            .api_client
            .get(format!("{}/stop-monitoring", self.api_base_url))
            .query(&[("MonitoringRef", &stop_id.prim())])
            .header("apiKey", &self.api_key)
            .timeout(self.request_timeout)
            .send()
            .await?;

        let status = res.status();
        if status != reqwest::StatusCode::OK {
            return Err(PrimError::Http { status });
        }
        let body = res.text().await?;
        Ok(serde_json::from_str(&body)?)
    }

    #[cfg(test)]
    pub(crate) fn for_test(base_url: String, timeout: std::time::Duration) -> Self {
        let mut client = Self::new("test-api-key".into());
        client.api_base_url = base_url;
        client.request_timeout = timeout;
        client
    }
}

pub fn parse_bus_info(json_value: serde_json::Value) -> Result<RealtimeReport, PrimError> {
    let service = json_value
        .get("Siri")
        .and_then(|siri| siri.get("ServiceDelivery"))
        .filter(|service| service.is_object())
        .ok_or(PrimError::InvalidEnvelope {
            field: "Siri.ServiceDelivery",
        })?;
    if service["Status"].as_bool() == Some(false) {
        return Err(PrimError::ServiceDeliveryFailed);
    }
    let deliveries = service
        .get("StopMonitoringDelivery")
        .and_then(|deliveries| deliveries.as_array())
        .ok_or(PrimError::InvalidEnvelope {
            field: "Siri.ServiceDelivery.StopMonitoringDelivery",
        })?;
    let mut report = RealtimeReport::default();
    for (delivery_index, delivery) in deliveries.iter().enumerate() {
        let issue = |field, reason: &str| ParseIssue {
            delivery_index,
            visit_index: None,
            field,
            reason: reason.into(),
            entry: delivery.clone(),
        };
        if delivery["Status"].as_bool() == Some(false) {
            report
                .issues
                .push(issue("Status", "delivery reported failure"));
            continue;
        }
        // An explicitly successful delivery may contain no visits.
        if delivery.get("MonitoredStopVisit").is_none()
            && delivery["Status"].as_bool() == Some(true)
        {
            continue;
        }
        let Some(visits) = delivery["MonitoredStopVisit"].as_array() else {
            report
                .issues
                .push(issue("MonitoredStopVisit", "expected an array"));
            continue;
        };
        for (visit_index, visit) in visits.iter().enumerate() {
            match parse_visit(visit, delivery_index, visit_index) {
                Ok(stop) => report.stops.push(stop),
                Err(issue) => report.issues.push(issue),
            }
        }
    }
    Ok(report)
}

fn parse_visit(
    visit: &serde_json::Value,
    delivery_index: usize,
    visit_index: usize,
) -> Result<RealtimeStop, ParseIssue> {
    let journey = &visit["MonitoredVehicleJourney"];
    let call = &journey["MonitoredCall"];
    let timestamp = |field: &'static str| -> Result<Timestamp, ParseIssue> {
        let issue = |reason| ParseIssue {
            delivery_index,
            visit_index: Some(visit_index),
            field,
            reason,
            entry: visit.clone(),
        };
        let value = call[field]
            .as_str()
            .ok_or_else(|| issue("expected a timestamp string".into()))?;
        value
            .parse::<Timestamp>()
            .map_err(|error| issue(error.to_string()))
    };
    // Some calls expose departure times only. Keep the expected and aimed times
    // from the same event so dwell time is not mistaken for a delay.
    let has_arrival_pair =
        !call["ExpectedArrivalTime"].is_null() && !call["AimedArrivalTime"].is_null();
    let has_departure_pair =
        !call["ExpectedDepartureTime"].is_null() && !call["AimedDepartureTime"].is_null();
    let (expected_field, aimed_field) = if !has_arrival_pair && has_departure_pair {
        ("ExpectedDepartureTime", "AimedDepartureTime")
    } else {
        ("ExpectedArrivalTime", "AimedArrivalTime")
    };
    let expected_arrival = timestamp(expected_field)?;
    let aimed_arrival = timestamp(aimed_field)?;
    let destination = journey["DestinationName"]
        .get(0)
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();
    let aimed_expected_minutes = expected_arrival.duration_since(aimed_arrival).as_mins();
    let status = if aimed_expected_minutes == 0 {
        RealtimeStopStatus::OnTime
    } else if aimed_expected_minutes > 0 {
        RealtimeStopStatus::Late(aimed_expected_minutes)
    } else {
        RealtimeStopStatus::Early(aimed_expected_minutes.abs())
    };
    Ok(RealtimeStop {
        expected_arrival,
        aimed_arrival,
        destination,
        status,
    })
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    pub struct MockPrim {
        pub requests: Arc<AtomicUsize>,
        task: tokio::task::JoinHandle<()>,
    }

    impl Drop for MockPrim {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    pub async fn mock_prim(
        status: u16,
        body: String,
        delay: Duration,
        timeout: Duration,
    ) -> (IdfmPrimClient, MockPrim) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = IdfmPrimClient::for_test(
            format!("http://{}", listener.local_addr().unwrap()),
            timeout,
        );
        let requests = Arc::new(AtomicUsize::new(0));
        let count = requests.clone();
        let task = tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut request = Vec::new();
                let mut buf = [0; 1024];
                loop {
                    let n = stream.read(&mut buf).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    request.extend_from_slice(&buf[..n]);
                    if request.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                count.fetch_add(1, Ordering::SeqCst);
                // Zero simulates an upstream disconnect without an HTTP response.
                if status == 0 {
                    continue;
                }
                tokio::time::sleep(delay).await;
                let response = format!(
                    "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });
        (client, MockPrim { requests, task })
    }

    pub fn visit(aimed: &str, expected: &str) -> serde_json::Value {
        serde_json::json!({ "MonitoredVehicleJourney": {
            "DestinationName": [{ "value": "Gare" }],
            "MonitoredCall": { "AimedArrivalTime": aimed, "ExpectedArrivalTime": expected }
        }})
    }

    pub fn envelope(deliveries: serde_json::Value) -> serde_json::Value {
        serde_json::json!({ "Siri": { "ServiceDelivery": { "StopMonitoringDelivery": deliveries } } })
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;
    use serde_json::json;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    #[test]
    fn accepts_the_departure_only_call_from_the_rejected_entry() {
        let entry = json!({
            "ItemIdentifier": "MLV:Item::MLV_268440852:LOC",
            "MonitoredVehicleJourney": {
                "DestinationName": [{ "value": "Gare de Lagny Th" }],
                "MonitoredCall": {
                    "AimedDepartureTime": "2026-10-02T14:30:00.000Z",
                    "ExpectedDepartureTime": "2026-10-02T14:36:11.000Z",
                    "ArrivalStatus": "",
                    "DepartureStatus": "delayed",
                    "VehicleAtStop": false
                }
            }
        });
        let report = parse_bus_info(envelope(json!([{ "MonitoredStopVisit": [entry] }]))).unwrap();
        assert!(report.issues.is_empty());
        assert_eq!(report.stops.len(), 1);
        let stop = &report.stops[0];
        assert_eq!(stop.aimed_arrival.to_string(), "2026-10-02T14:30:00Z");
        assert_eq!(stop.expected_arrival.to_string(), "2026-10-02T14:36:11Z");
        assert_eq!(stop.destination, "Gare de Lagny Th");
        assert_eq!(stop.status.to_string(), "late by 6'");
    }

    #[test]
    fn uses_complete_time_pairs_and_keeps_invalid_timestamps_strict() {
        let mut entry = visit("2026-10-02T14:28:00Z", "2026-10-02T14:29:00Z");
        let call = &mut entry["MonitoredVehicleJourney"]["MonitoredCall"];
        call["AimedDepartureTime"] = json!("2026-10-02T14:30:00Z");
        call["ExpectedDepartureTime"] = json!("2026-10-02T14:36:11Z");
        let stop = parse_visit(&entry, 0, 0).unwrap();
        assert_eq!(stop.status.to_string(), "late by 1'");

        // An incomplete arrival pair falls back to the complete departure pair.
        entry["MonitoredVehicleJourney"]["MonitoredCall"]["ExpectedArrivalTime"] = json!(null);
        let stop = parse_visit(&entry, 0, 0).unwrap();
        assert_eq!(stop.aimed_arrival.to_string(), "2026-10-02T14:30:00Z");
        assert_eq!(stop.status.to_string(), "late by 6'");

        // Do not combine an aimed arrival with an expected departure.
        entry["MonitoredVehicleJourney"]["MonitoredCall"]
            .as_object_mut()
            .unwrap()
            .remove("AimedDepartureTime");
        assert!(parse_visit(&entry, 0, 0).is_err());

        // Report malformed departure values with the correct field and entry.
        entry["MonitoredVehicleJourney"]["MonitoredCall"]["AimedDepartureTime"] =
            json!("bad timestamp");
        let issue = parse_visit(&entry, 0, 0).unwrap_err();
        assert_eq!(issue.field, "AimedDepartureTime");
        assert_eq!(issue.entry, entry);
    }

    #[test]
    fn keeps_valid_visits_before_and_after_invalid_timestamps() {
        let good = visit("2024-10-27T00:30:00Z", "2024-10-27T00:35:00Z");
        let mut missing = good.clone();
        missing["MonitoredVehicleJourney"]["MonitoredCall"]
            .as_object_mut()
            .unwrap()
            .remove("ExpectedArrivalTime");
        let bad_aimed = visit("invalid", "2024-10-27T00:35:00Z");
        let bad_expected = visit("2024-10-27T00:30:00Z", "invalid");
        let report = parse_bus_info(envelope(json!([
            { "MonitoredStopVisit": [good, missing, bad_aimed, bad_expected, good] }
        ])))
        .unwrap();
        assert_eq!(report.stops.len(), 2);
        assert_eq!(report.issues.len(), 3);
        assert_eq!(report.issues[0].visit_index, Some(1));
        assert_eq!(report.issues[0].field, "ExpectedArrivalTime");
        assert_eq!(report.issues[0].entry, missing);
        assert_eq!(report.issues[1].entry, bad_aimed);
        assert_eq!(report.issues[2].entry, bad_expected);
        assert_eq!(report.issues[1].field, "AimedArrivalTime");
        assert_eq!(report.issues[2].field, "ExpectedArrivalTime");
    }

    #[test]
    fn keeps_sibling_deliveries_when_one_is_malformed_or_failed() {
        let good = json!({ "MonitoredStopVisit": [visit("2024-10-27T00:30:00Z", "2024-10-27T00:35:00Z")] });
        let report = parse_bus_info(envelope(json!([
            good, null, { "MonitoredStopVisit": {} }, { "Status": false }, good
        ])))
        .unwrap();
        assert_eq!(report.stops.len(), 2);
        assert_eq!(report.issues.len(), 3);
        assert_eq!(report.issues[0].delivery_index, 1);
        assert_eq!(report.issues[0].visit_index, None);
        assert_eq!(report.issues[2].field, "Status");
        assert_eq!(report.issues[0].entry, json!(null));
        assert_eq!(report.issues[1].entry, json!({ "MonitoredStopVisit": {} }));
        assert_eq!(report.issues[2].entry, json!({ "Status": false }));
    }

    #[test]
    fn distinguishes_empty_deliveries_from_invalid_envelopes() {
        for deliveries in [
            json!([]),
            json!([{ "MonitoredStopVisit": [] }]),
            json!([{ "Status": true }]),
        ] {
            let report = parse_bus_info(envelope(deliveries)).unwrap();
            assert!(report.stops.is_empty());
            assert!(report.issues.is_empty());
        }
        for invalid in [
            json!({}),
            json!({"Siri":{"ServiceDelivery":[]}}),
            envelope(json!({})),
        ] {
            assert!(matches!(
                parse_bus_info(invalid),
                Err(PrimError::InvalidEnvelope { .. })
            ));
        }
        assert!(matches!(
            parse_bus_info(json!({"Siri":{"ServiceDelivery":{"Status":false}}})),
            Err(PrimError::ServiceDeliveryFailed)
        ));
    }

    #[tokio::test]
    async fn classifies_http_json_envelope_and_transport_failures() {
        for (status, body, delay) in [
            (404, "missing", Duration::ZERO),
            (503, "unavailable", Duration::ZERO),
            (200, "invalid json", Duration::ZERO),
            (200, "{}", Duration::ZERO),
            (0, "", Duration::ZERO),
            (200, "{}", Duration::from_secs(1)),
        ] {
            let (client, _server) =
                mock_prim(status, body.into(), delay, Duration::from_millis(100)).await;
            let error = client.get_next_busses("1234").await.unwrap_err();
            match (status, body, delay.is_zero()) {
                (404 | 503, _, _) => assert!(
                    matches!(error, PrimError::Http { status: actual } if actual.as_u16() == status)
                ),
                (200, "invalid json", _) => assert!(matches!(error, PrimError::InvalidJson(_))),
                (200, "{}", true) => assert!(matches!(error, PrimError::InvalidEnvelope { .. })),
                (0, _, _) => assert!(matches!(error, PrimError::Transport(_))),
                (_, _, false) => assert!(
                    matches!(error, PrimError::Transport(ref source) if source.is_timeout())
                ),
                _ => unreachable!(),
            }
        }
    }

    #[tokio::test]
    async fn caches_partial_and_empty_results_but_not_wholly_rejected_responses() {
        let good = visit("2024-10-27T00:30:00Z", "2024-10-27T00:35:00Z");
        for (deliveries, expected_requests, expected_stops, expected_issues) in [
            (json!([{ "MonitoredStopVisit": [good, {}] }]), 1, 1, 1),
            (json!([]), 1, 0, 0),
            (json!([{ "MonitoredStopVisit": [{}] }]), 2, 0, 1),
        ] {
            let (client, server) = mock_prim(
                200,
                envelope(deliveries).to_string(),
                Duration::ZERO,
                Duration::from_secs(1),
            )
            .await;
            for _ in 0..2 {
                let report = client.get_next_busses("1234").await.unwrap();
                assert_eq!(report.stops.len(), expected_stops);
                assert_eq!(report.issues.len(), expected_issues);
            }
            assert_eq!(server.requests.load(Ordering::SeqCst), expected_requests);
        }
    }

    #[test]
    fn cache_ttl_is_shortened_when_a_bus_is_imminent() {
        let now: jiff::Timestamp = "2024-10-27T00:30:00Z".parse().unwrap();
        let report_at = |offset_secs: i64| RealtimeReport {
            stops: vec![crate::RealtimeStop {
                expected_arrival: now + jiff::SignedDuration::from_secs(offset_secs),
                aimed_arrival: now,
                destination: "Gare".into(),
                status: crate::RealtimeStopStatus::OnTime,
            }],
            issues: vec![],
        };
        assert_eq!(RealtimeReport::default().cache_ttl(now), CACHE_TTL);
        assert_eq!(report_at(60).cache_ttl(now), CACHE_TTL);
        assert_eq!(report_at(59).cache_ttl(now), IMMINENT_CACHE_TTL);
        assert_eq!(report_at(-30).cache_ttl(now), IMMINENT_CACHE_TTL);
    }
}
