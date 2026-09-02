use chrono::Datelike;
use jiff::civil::Date;
use rayon::prelude::*;

pub trait GtfsExtract {
    fn extract_gtfs_route(
        &mut self,
        gtfs: gtfs_structures::Gtfs,
        route_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>>;
}

impl GtfsExtract for morningstar_model::TimeTable {
    fn extract_gtfs_route(
        &mut self,
        gtfs: gtfs_structures::Gtfs,
        route_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.timezone = gtfs.agencies[0].timezone.clone();
        let mut journeys: Vec<_> = gtfs
            .trips
            .par_iter()
            .filter(|(_, candidate_trip)| candidate_trip.route_id == route_id)
            .map(|(_, value)| value)
            .filter_map(trip_convert)
            .collect();
        if journeys.is_empty() {
            return Err("no trip was available".into());
        }
        let service_ids: std::collections::HashSet<_> = journeys
            .iter()
            .map(|journey| journey.service_id.as_str())
            .collect();
        service_ids
            .iter()
            .for_each(|service_id| extract_pattern_and_exceptions(self, &gtfs, service_id));
        self.journeys.append(&mut journeys);
        self.sort_journeys_and_stops();
        Ok(())
    }
}

fn extract_pattern_and_exceptions(
    tt: &mut morningstar_model::TimeTable,
    gtfs: &gtfs_structures::Gtfs,
    service_id: &str,
) {
    if let Some(calendar) = gtfs.calendar.get(service_id) {
        match callendar_to_pattern(calendar) {
            Ok(pattern) => {
                tt.service_patterns.insert(service_id.to_owned(), pattern);
            }
            Err(error) => {
                eprintln!("warning: skipping calendar for service {service_id}: {error}");
            }
        }
    }
    if let Some(callendar_dates) = gtfs.calendar_dates.get(service_id) {
        for callendar_date in callendar_dates {
            let date = match chrono_to_jiff_date(callendar_date.date) {
                Ok(date) => date,
                Err(error) => {
                    eprintln!(
                        "warning: skipping calendar exception for service {service_id}: {error}"
                    );
                    continue;
                }
            };
            let excpetion = morningstar_model::ServiceException {
                date,
                exception_type: match callendar_date.exception_type {
                    gtfs_structures::Exception::Added => morningstar_model::Exception::Added,
                    gtfs_structures::Exception::Deleted => morningstar_model::Exception::Deleted,
                },
            };
            tt.excpetions.insert(service_id.to_owned(), excpetion);
        }
    }
}

fn chrono_to_jiff_date(date: chrono::NaiveDate) -> Result<Date, String> {
    let year = i16::try_from(date.year())
        .map_err(|_| format!("date {date} is outside Jiff's supported year range"))?;
    let month =
        i8::try_from(date.month()).map_err(|_| format!("date {date} has an invalid month"))?;
    let day = i8::try_from(date.day()).map_err(|_| format!("date {date} has an invalid day"))?;

    Date::new(year, month, day).map_err(|error| format!("could not convert date {date}: {error}"))
}

fn callendar_to_pattern(
    calendar: &gtfs_structures::Calendar,
) -> Result<morningstar_model::ServicePattern, String> {
    use morningstar_model::WeekdayFlags;
    let mut pattern = morningstar_model::ServicePattern {
        weekdays: WeekdayFlags::NEVER,
        start_date: chrono_to_jiff_date(calendar.start_date)?,
        end_date: chrono_to_jiff_date(calendar.end_date)?,
    };
    if calendar.monday {
        pattern.weekdays.set(WeekdayFlags::MONDAY, true);
    }
    if calendar.tuesday {
        pattern.weekdays.set(WeekdayFlags::TUESDAY, true);
    }
    if calendar.wednesday {
        pattern.weekdays.set(WeekdayFlags::WEDNESDAY, true);
    }
    if calendar.thursday {
        pattern.weekdays.set(WeekdayFlags::THURSDAY, true);
    }
    if calendar.friday {
        pattern.weekdays.set(WeekdayFlags::FRIDAY, true);
    }
    if calendar.saturday {
        pattern.weekdays.set(WeekdayFlags::SATURDAY, true);
    }
    if calendar.sunday {
        pattern.weekdays.set(WeekdayFlags::SUNDAY, true);
    }
    Ok(pattern)
}

fn trip_convert(trip: &gtfs_structures::Trip) -> Option<morningstar_model::Journey> {
    let stops: Vec<_> = trip
        .stop_times
        .iter()
        .filter_map(stop_time_convert)
        .collect();

    if stops.is_empty() {
        None
    } else {
        Some(morningstar_model::Journey {
            service_id: trip.service_id.clone(),
            stops,
        })
    }
}

fn stop_time_convert(stop_time: &gtfs_structures::StopTime) -> Option<morningstar_model::StopTime> {
    let stop_name = stop_time.stop.name.clone()?;
    let stop_id = stop_time.stop.id.clone();
    let seconds_from_midnight = stop_time.arrival_time.or(stop_time.departure_time)?;
    let hour = i8::try_from(seconds_from_midnight / 3_600).ok()?;
    let minute = i8::try_from((seconds_from_midnight % 3_600) / 60).ok()?;
    let second = i8::try_from(seconds_from_midnight % 60).ok()?;
    let time_of_day = jiff::civil::Time::new(hour, minute, second, 0).ok()?;
    Some(morningstar_model::StopTime {
        time: time_of_day,
        stop_name,
        stop_id,
    })
}

#[cfg(test)]
mod tests {
    use super::chrono_to_jiff_date;
    use chrono::NaiveDate;
    use jiff::civil::date;

    #[test]
    fn converts_chrono_date_to_jiff_without_formatting() {
        let chrono_date = NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();

        assert_eq!(chrono_to_jiff_date(chrono_date), Ok(date(2024, 2, 29)));
    }

    #[test]
    fn rejects_dates_outside_jiffs_supported_range() {
        let chrono_date = NaiveDate::from_ymd_opt(10_000, 1, 1).unwrap();

        let error = chrono_to_jiff_date(chrono_date).unwrap_err();
        assert!(error.contains("could not convert date +10000-01-01"));
    }
}
