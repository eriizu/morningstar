use chrono::prelude::*;

impl super::Timetable {
    fn runs_today_uncached(&self, service_id: &str) -> bool {
        match self.runs_by_exception(service_id) {
            Some(super::my_gtfs_structs::Exception::Added) => {
                println!("{service_id} passes by exception");
                true
            }
            Some(super::my_gtfs_structs::Exception::Deleted) => {
                println!("{service_id} rejected by exception");
                false
            }
            None => self.runs_on_interval_weekday(service_id).unwrap_or(false),
        }
    }

    pub fn runs_today(&self, service_id: &str) -> bool {
        if self.running_services_cache.borrow().contains(service_id) {
            return true;
        } else if self
            .non_running_services_cache
            .borrow()
            .contains(service_id)
        {
            return false;
        }
        let runs = self.runs_today_uncached(service_id);
        if runs {
            self.running_services_cache
                .borrow_mut()
                .insert(service_id.to_owned());
        } else {
            self.non_running_services_cache
                .borrow_mut()
                .insert(service_id.to_owned());
        }
        runs
    }

    fn runs_by_exception(&self, service_id: &str) -> Option<super::my_gtfs_structs::Exception> {
        if let Some(exceptions) = self.calendar_dates.get(service_id) {
            exceptions
                .iter()
                .filter(|exception| exception.date == self.today)
                .map(|exception| exception.exception_type)
                .fold(Option::None, |acc, excp_type| {
                    if let Some(val) = acc {
                        // INFO: if we have conflicting exceptions we act as if we don't have an
                        // exception.
                        if val != excp_type {
                            eprintln!(
                            "warning: conflicting exceptions detected (service_id: {service_id})"
                        );
                            return Option::None;
                        }
                    }
                    Option::Some(excp_type)
                })
        } else {
            None
        }
    }

    fn runs_on_interval_weekday(&self, service_id: &str) -> Option<bool> {
        let gtfs_cal = self.calendar.get(service_id)?;
        if self.today < gtfs_cal.start_date || self.today > gtfs_cal.end_date {
            // println!("{service_id} date ranges don't match today's date.");
            return None;
        }
        let runs_today = match self.now.weekday() {
            chrono::Weekday::Mon if gtfs_cal.monday => true,
            chrono::Weekday::Tue if gtfs_cal.tuesday => true,
            chrono::Weekday::Wed if gtfs_cal.wednesday => true,
            chrono::Weekday::Thu if gtfs_cal.thursday => true,
            chrono::Weekday::Fri if gtfs_cal.friday => true,
            chrono::Weekday::Sat if gtfs_cal.saturday => true,
            chrono::Weekday::Sun if gtfs_cal.sunday => true,
            _ => false,
        };
        if runs_today {
            println!("{service_id} runs today on a regular basis");
        }
        Some(runs_today)
    }
}
