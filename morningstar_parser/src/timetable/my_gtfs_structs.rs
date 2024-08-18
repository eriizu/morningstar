use structural_convert::StructuralConvert;

#[derive(Clone, serde::Deserialize, serde::Serialize, Debug, StructuralConvert)]
#[convert(from(gtfs_structures::Calendar))]
pub struct Calendar {
    pub id: String,
    pub monday: bool,
    pub tuesday: bool,
    pub wednesday: bool,
    pub thursday: bool,
    pub friday: bool,
    pub saturday: bool,
    pub sunday: bool,
    pub start_date: chrono::NaiveDate,
    pub end_date: chrono::NaiveDate,
}
#[derive(Clone, serde::Deserialize, serde::Serialize, Debug, StructuralConvert)]
#[convert(from(gtfs_structures::Stop))]
pub struct Stop {
    pub id: String,
    pub code: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    // pub location_type: LocationType,
    pub parent_station: Option<String>,
    pub zone_id: Option<String>,
    pub url: Option<String>,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    pub timezone: Option<String>,
    // pub wheelchair_boarding: Availability,
    pub level_id: Option<String>,
    pub platform_code: Option<String>,
    // pub transfers: Vec<StopTransfer>,
    // pub pathways: Vec<Pathway>,
    pub tts_name: Option<String>,
}

#[derive(Clone, serde::Deserialize, serde::Serialize, Debug, StructuralConvert)]
#[convert(from(gtfs_structures::Route))]
pub struct Route {
    pub id: String,
    pub short_name: Option<String>,
    pub long_name: Option<String>,
    pub desc: Option<String>,
    // pub route_type: RouteType,
    pub url: Option<String>,
    pub agency_id: Option<String>,
    pub order: Option<u32>,
    // pub color: RGB8,
    // pub text_color: RGB8,
    // pub continuous_pickup: ContinuousPickupDropOff,
    // pub continuous_drop_off: ContinuousPickupDropOff,
}

#[derive(Clone, serde::Deserialize, serde::Serialize, Debug, StructuralConvert)]
#[convert(from(gtfs_structures::CalendarDate))]
pub struct CalendarDate {
    pub service_id: String,
    pub date: chrono::NaiveDate,
    pub exception_type: Exception,
}

#[derive(
    serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy, StructuralConvert,
)]
#[convert(from(gtfs_structures::Exception))]
pub enum Exception {
    Added,
    Deleted,
}
// impl std::convert::From<&gtfs_structures::Calendar> for Calendar {
//     fn from(value: &gtfs_structures::Calendar) -> Self {
//         Self {
//             id: value.id.clone(),
//             monday: value.monday,
//             tuesday: value.tuesday,
//             wednesday: value.wednesday,
//             thursday: value.thursday,
//             friday: value.friday,
//             saturday: value.saturday,
//             sunday: value.sunday,
//             start_date: value.start_date,
//             end_date: value.end_date,
//         }
//     }
// }
