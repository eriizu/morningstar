use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

mod weekday_flags;
pub use weekday_flags::WeekdayFlags;

/// Journeys and stops
#[derive(Serialize, Deserialize)]
pub struct TimeTable {
    pub journeys: Vec<Journey>,
    pub excpetions: multimap::MultiMap<String, ServiceException>,
    pub service_patterns: HashMap<String, ServicePattern>,
}

impl TimeTable {
    pub fn new() -> Self {
        TimeTable::default()
    }

    pub fn sort_journeys_and_stops(&mut self) {
        self.journeys
            .iter_mut()
            .map(|journey| &mut journey.stops)
            .for_each(|stops| stops.sort_by(|lhs, rhs| lhs.time.cmp(&rhs.time)));
        self.journeys
            .sort_by(|lhs, rhs| lhs.stops[0].time.cmp(&rhs.stops[0].time));
    }

    /// Iterator on journeys that run on provided date.
    pub fn get_journeys_for_day<'a>(
        &'a self,
        day: &'a chrono::NaiveDate,
    ) -> impl Iterator<Item = &'a Journey> {
        self.journeys
            .iter()
            .filter_map(|journey| self.filter_map_journey_on_date(journey, day))
    }

    /// Retuns the given journey Some variant if it runs on provided day,
    /// checking service patterns and exceptions.
    fn filter_map_journey_on_date<'a>(
        &'a self,
        journey: &'a Journey,
        day: &'a chrono::NaiveDate,
    ) -> Option<&'a Journey> {
        if let Some(kind) = self.get_exception_kind_for_day(&journey.service_id, day) {
            return match kind {
                Exception::Added => Some(journey),
                Exception::Deleted => None,
            };
        }
        let pattern = self.service_patterns.get(&journey.service_id)?;
        if pattern.start_date.le(day)
            && pattern.end_date.ge(day)
            && weekday_flags::runs_on_date(day, pattern.weekdays)
        {
            Some(journey)
        } else {
            None
        }
    }

    /// Iterator on stoptime tuples for stop served on provided day for a trip
    /// in between stop a and b. Names must be exact.
    pub fn get_day_stoptimes_from_a_to_b<'a>(
        &'a self,
        day: &'a chrono::NaiveDate,
        a: &'a str,
        b: &'a str,
    ) -> impl Iterator<Item = (&'a StopTime, &'a StopTime)> {
        let today_journeys = self.get_journeys_for_day(day);
        today_journeys.filter_map(move |journey| {
            match (
                journey.stops.iter().find(|stop| stop.stop_name == a),
                journey.stops.iter().find(|stop| stop.stop_name == b),
            ) {
                (Some(stop1), Some(stop2)) if stop1.time < stop2.time => Some((stop1, stop2)),
                _ => None,
            }
        })
    }

    /// Iterator on stoptimes for stop served on provided day for a trip
    /// from a stop name. Names must be exact.
    pub fn get_day_stoptimes_from_stop<'a>(
        &'a self,
        day: &'a chrono::NaiveDate,
        stop_name: &'a str,
    ) -> impl Iterator<Item = &'a StopTime> {
        let today_journeys = self.get_journeys_for_day(day);
        today_journeys.filter_map(move |journey| {
            journey
                .stops
                .iter()
                .find(|stop| stop.stop_name == stop_name)
        })
    }

    pub fn get_stops_served_on_day<'a>(&'a self, day: &'a chrono::NaiveDate) -> HashSet<&'a str> {
        self.get_journeys_for_day(day)
            .flat_map(|journey| journey.stops.iter().map(|stop| stop.stop_name.as_str()))
            .collect()
    }

    fn get_exception_kind_for_day(
        &self,
        service_id: &str,
        day: &chrono::NaiveDate,
    ) -> Option<Exception> {
        let (added, deleted) = self
            .excpetions
            .get_vec(service_id)?
            .iter()
            .filter(|exception| exception.date == *day)
            .fold((false, false), |acc, excpetion| {
                match excpetion.exception_type {
                    Exception::Added => (true, acc.1),
                    Exception::Deleted => (acc.0, true),
                }
            });
        if added == deleted {
            None
        } else if added {
            Some(Exception::Added)
        } else {
            Some(Exception::Deleted)
        }
    }
}

impl Default for TimeTable {
    fn default() -> Self {
        Self {
            journeys: vec![],
            excpetions: multimap::MultiMap::new(),
            service_patterns: HashMap::new(),
        }
    }
}

/// One bus journey, with all its stops and bitflags indicating when does
/// it run.
#[derive(Debug, Serialize, Deserialize)]
pub struct Journey {
    pub service_id: String,
    pub stops: Vec<StopTime>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StopTime {
    pub time: chrono::NaiveTime,
    pub stop_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServicePattern {
    pub weekdays: WeekdayFlags,
    pub start_date: chrono::NaiveDate,
    pub end_date: chrono::NaiveDate,
}

// #[derive(Clone, serde::Deserialize, serde::Serialize, Debug, StructuralConvert)]
// #[convert(from(gtfs_structures::CalendarDate))]
#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceException {
    pub date: chrono::NaiveDate,
    pub exception_type: Exception,
}

// #[derive(
//     serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy, StructuralConvert,
// )]
// #[convert(from(gtfs_structures::Exception))]
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum Exception {
    Added,
    Deleted,
}

#[cfg(test)]
mod test {
    use super::*;
    use chrono::prelude::*;

    fn sample_tt() -> TimeTable {
        let mut tt = super::TimeTable::new();
        let mut service_pattern = super::ServicePattern {
            weekdays: WeekdayFlags::WORKDAYS,
            start_date: NaiveDate::from_yo_opt(2024, 1).unwrap(),
            end_date: NaiveDate::from_yo_opt(2024, 75).unwrap(),
        };
        tt.service_patterns
            .insert("wd1".to_owned(), service_pattern.clone());
        service_pattern.weekdays = WeekdayFlags::WEEKENDS;
        tt.service_patterns
            .insert("we1".to_owned(), service_pattern.clone());
        let mut wd_stops = vec![
            StopTime {
                time: chrono::NaiveTime::from_hms_opt(14, 0, 0).unwrap(),
                stop_name: "Église".to_owned(),
            },
            StopTime {
                time: chrono::NaiveTime::from_hms_opt(14, 6, 0).unwrap(),
                stop_name: "Marché".to_owned(),
            },
            StopTime {
                time: chrono::NaiveTime::from_hms_opt(14, 9, 0).unwrap(),
                stop_name: "Potato Factory".to_owned(),
            },
            StopTime {
                time: chrono::NaiveTime::from_hms_opt(14, 15, 0).unwrap(),
                stop_name: "Gare".to_owned(),
            },
        ];
        tt.journeys.push(Journey {
            service_id: "wd1".to_owned(),
            stops: wd_stops.clone(),
        });
        wd_stops[0].time = chrono::NaiveTime::from_hms_opt(15, 0, 0).unwrap();
        wd_stops[1].time = chrono::NaiveTime::from_hms_opt(15, 6, 0).unwrap();
        wd_stops[2].time = chrono::NaiveTime::from_hms_opt(15, 6, 0).unwrap();
        wd_stops[3].time = chrono::NaiveTime::from_hms_opt(15, 15, 0).unwrap();
        tt.journeys.push(Journey {
            service_id: "wd1".to_owned(),
            stops: wd_stops.clone(),
        });
        let mut we_stops = vec![
            StopTime {
                time: chrono::NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
                stop_name: "Église".to_owned(),
            },
            StopTime {
                time: chrono::NaiveTime::from_hms_opt(16, 6, 0).unwrap(),
                stop_name: "Marché".to_owned(),
            },
            StopTime {
                time: chrono::NaiveTime::from_hms_opt(16, 9, 0).unwrap(),
                stop_name: "Terrain d'airsoft".to_owned(),
            },
            StopTime {
                time: chrono::NaiveTime::from_hms_opt(16, 15, 0).unwrap(),
                stop_name: "Gare".to_owned(),
            },
        ];
        tt.journeys.push(Journey {
            service_id: "we1".to_owned(),
            stops: we_stops.clone(),
        });
        we_stops[0].time = chrono::NaiveTime::from_hms_opt(15, 0, 0).unwrap();
        we_stops[1].time = chrono::NaiveTime::from_hms_opt(15, 6, 0).unwrap();
        we_stops[2].time = chrono::NaiveTime::from_hms_opt(15, 6, 0).unwrap();
        we_stops[3].time = chrono::NaiveTime::from_hms_opt(15, 15, 0).unwrap();
        tt.journeys.push(Journey {
            service_id: "we1".to_owned(),
            stops: we_stops.clone(),
        });
        tt
    }

    #[test]
    fn inside_operating_range() {
        let mut tt = sample_tt();
        internal_inside_operating_range(&mut tt);
    }

    fn internal_inside_operating_range(tt: &mut TimeTable) {
        let we_day = NaiveDate::from_yo_opt(2024, 7).unwrap();
        let wd_day = NaiveDate::from_yo_opt(2024, 8).unwrap();
        let journeys: Vec<_> = tt.get_journeys_for_day(&wd_day).collect();
        for item in journeys {
            let count = item
                .stops
                .iter()
                .map(|stop_time| &stop_time.stop_name)
                .filter(|stop_name| *stop_name == "Potato Factory")
                .count();
            assert_eq!(count, 1);
        }
        let journeys = tt.get_journeys_for_day(&we_day);
        for item in journeys {
            let count = item
                .stops
                .iter()
                .map(|stop_time| &stop_time.stop_name)
                .filter(|stop_name| *stop_name == "Terrain d'airsoft")
                .count();
            assert_eq!(count, 1);
        }
    }

    #[test]
    fn outside_operating_range() {
        let mut tt = sample_tt();
        internal_outside_operating_range(&mut tt);
    }

    fn internal_outside_operating_range(tt: &mut TimeTable) {
        let we_day = NaiveDate::from_yo_opt(2024, 222).unwrap();
        let wd_day = NaiveDate::from_yo_opt(2024, 223).unwrap();
        let journeys = tt.get_journeys_for_day(&wd_day).count();
        assert_eq!(journeys, 0);
        let journeys = tt.get_journeys_for_day(&we_day).count();
        assert_eq!(journeys, 0);
        let day = NaiveDate::from_yo_opt(2024, 76).unwrap();
        let journeys = tt.get_journeys_for_day(&day).count();
        assert_eq!(journeys, 0);
    }

    #[test]
    fn serde() {
        let serialised = {
            let tt = sample_tt();
            serde_json::to_string(&tt).unwrap()
        };
        let mut tt: TimeTable = serde_json::from_str(&serialised).unwrap();
        internal_inside_operating_range(&mut tt);
        internal_outside_operating_range(&mut tt);
    }

    #[test]
    fn a_to_b_journey() {
        let tt = sample_tt();
        let day = NaiveDate::from_yo_opt(2024, 7).unwrap();
        let stoptimes: Vec<_> = tt
            .get_day_stoptimes_from_a_to_b(&day, "Marché", "Gare")
            .collect();
        let size = stoptimes.len();
        assert_eq!(size, 2);
        for stoptime in stoptimes {
            assert_eq!(stoptime.0.stop_name, "Marché");
            assert_eq!(stoptime.1.stop_name, "Gare");
        }
    }

    #[test]
    fn a_to_b_journey_wrong_way() {
        let tt = sample_tt();
        let day = NaiveDate::from_yo_opt(2024, 7).unwrap();
        let stoptimes: Vec<_> = tt
            .get_day_stoptimes_from_a_to_b(&day, "Gare", "Marché")
            .collect();
        let size = stoptimes.len();
        assert_eq!(size, 0);
    }

    #[test]
    fn a_to_b_journey_non_existant_dest() {
        let tt = sample_tt();
        let day = NaiveDate::from_yo_opt(2024, 7).unwrap();
        let stoptimes: Vec<_> = tt
            .get_day_stoptimes_from_a_to_b(&day, "Marché", "Arrêt qui n'existe pas")
            .collect();
        let size = stoptimes.len();
        assert_eq!(size, 0);
    }

    #[test]
    fn a_to_b_journey_non_existant_start() {
        let tt = sample_tt();
        let day = NaiveDate::from_yo_opt(2024, 7).unwrap();
        let stoptimes: Vec<_> = tt
            .get_day_stoptimes_from_a_to_b(&day, "Arrêt qui n'existe pas", "Marché")
            .collect();
        let size = stoptimes.len();
        assert_eq!(size, 0);
    }

    #[test]
    fn day_stop_names() {
        let tt = sample_tt();
        let day = NaiveDate::from_yo_opt(2024, 7).unwrap();
        let stop_names = tt.get_stops_served_on_day(&day);
        assert_eq!(
            stop_names,
            ["Église", "Marché", "Gare", "Terrain d'airsoft"].into()
        );
        let day = NaiveDate::from_yo_opt(2024, 8).unwrap();
        let stop_names = tt.get_stops_served_on_day(&day);
        assert_eq!(
            stop_names,
            ["Église", "Marché", "Gare", "Potato Factory"].into()
        );
    }
}
