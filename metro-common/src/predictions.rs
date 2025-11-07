use core::{cmp::Ordering, fmt::{self, Display}, num::ParseIntError, str::FromStr};

use crate::{MetroString, MetroVec, ReMetroError};
use serde::{Deserialize, Serialize};

/// Parses a decimal `u8` without using `alloc` or `std`.
#[inline]
fn parse_u8_ascii(s: &str) -> Option<u8> {
    let mut val: u16 = 0;
    if s.is_empty() || s.len() > 3 {
        return None;
    }
    for &b in s.as_bytes() {
        if !b.is_ascii_digit() {
            return None;
        }
        val = val * 10 + (b - b'0') as u16;
        if val > u8::MAX as u16 {
            return None;
        }
    }
    Some(val as u8)
}

/// Return `true` if string is exactly `"---"`
#[inline]
fn is_dash_triplet(s: &str) -> bool {
    matches!(s.as_bytes(), [b'-', b'-', b'-'])
}

#[inline]
fn is_single_dash(s: &str) -> bool {
    matches!(s.as_bytes(), [b'-'])
}

/// Represents the Metro Line
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum TrainLine {
    Red,
    Orange,
    Blue,
    Green,
    Yellow,
    Silver,
    NoPassengers,
}

impl TrainLine {
    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            TrainLine::Red => "RD",
            TrainLine::Orange => "OR",
            TrainLine::Blue => "BL",
            TrainLine::Green => "GR",
            TrainLine::Yellow => "YL",
            TrainLine::Silver => "SV",
            TrainLine::NoPassengers => "No",
        }
    }

    #[inline]
    pub fn from_code(s: &str) -> Result<Self, ReMetroError> {
        match s {
            "RD" => Ok(TrainLine::Red),
            "OR" => Ok(TrainLine::Orange),
            "BL" => Ok(TrainLine::Blue),
            "GR" => Ok(TrainLine::Green),
            "YL" => Ok(TrainLine::Yellow),
            "SV" => Ok(TrainLine::Silver),
            "No" | "--" => Ok(TrainLine::NoPassengers),
            _ => {
                eprintln!("Invalid line code encountered: {}", s);
                Err(ReMetroError::LineConversion)
            },
        }
    }
}

impl Ord for TrainLine {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.code().cmp(other.code())
    }
}

impl PartialOrd for TrainLine {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for TrainLine {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl FromStr for TrainLine {
    type Err = ReMetroError;
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TrainLine::from_code(s)
    }
}

/// Represents statuses like ARR / BRD / DLY
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum OtherStatus {
    Arriving,
    Boarding,
    Delayed,
}

impl OtherStatus {
    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            OtherStatus::Arriving => "ARR",
            OtherStatus::Boarding => "BRD",
            OtherStatus::Delayed => "DLY",
        }
    }

    #[inline]
    pub fn from_code(s: &str) -> Result<Self, ReMetroError> {
        match s {
            "ARR" => Ok(OtherStatus::Arriving),
            "BRD" => Ok(OtherStatus::Boarding),
            "DLY" => Ok(OtherStatus::Delayed),
            _ => Err(ReMetroError::OtherStatusConversion),
        }
    }
}

impl fmt::Display for OtherStatus {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl FromStr for OtherStatus {
    type Err = ReMetroError;
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        OtherStatus::from_code(s)
    }
}

/// Represents timing prediction
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum TrainMins {
    Min(u8),
    Other(OtherStatus),
    Unknown,
}

impl TrainMins {
    #[inline]
    fn sort_key(&self) -> (u8, u8) {
        match *self {
            TrainMins::Other(OtherStatus::Boarding) => (0, 0),
            TrainMins::Other(OtherStatus::Arriving) => (1, 0),
            TrainMins::Min(n)                   => (2, n),
            TrainMins::Other(OtherStatus::Delayed)  => (3, 0),
            TrainMins::Unknown                      => (4, 0),
        }
    }
}

impl Ord for TrainMins {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.sort_key().cmp(&other.sort_key())
    }
}

impl PartialOrd for TrainMins {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl TrainMins {
    #[inline]
    pub fn from_display_str(s: &str) -> Result<Self, ReMetroError> {
        if s.is_empty() {
            return Ok(TrainMins::Unknown);
        }
        if is_dash_triplet(s) {
            return Ok(TrainMins::Unknown);
        }
        if let Some(n) = parse_u8_ascii(s) {
            return Ok(TrainMins::Min(n));
        }
        match OtherStatus::from_code(s) {
            Ok(s) => Ok(TrainMins::Other(s)),
            Err(_) => Err(ReMetroError::TrainPredictionConversion),
        }
    }
}

/// Represents a change in arrival time for a specific train
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct TrainMinuteChange {
    /// Index of the train in the current lineup (0-based)
    pub train_index: usize,
    /// New arrival time/status
    pub new_minutes: TrainMins,
    /// Previous arrival time/status for comparison
    pub old_minutes: TrainMins,
}

/// Subscription update indicating trains for a station/platform have changed
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub enum TrainUpdate {
    /// Full update with the current list of trains
    Full {
        trains: Vec<Train>,
    },
    /// Incremental update with changes to specific trains
    Incremental {
        changes: Vec<TrainMinuteChange>,
    }
}

impl fmt::Display for TrainMins {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrainMins::Min(n) => write!(f, "{n}"),
            TrainMins::Other(s) => f.write_str(s.code()),
            TrainMins::Unknown => f.write_str("---"),
        }
    }
}

impl FromStr for TrainMins {
    type Err = ReMetroError;
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TrainMins::from_display_str(s)
    }
}

/// Represents how many cars a train has
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum TrainCars {
    Num(u8),
    Unknown,
}

impl TrainCars {
    #[inline]
    pub fn from_display_str(s: &str) -> Result<Self, ReMetroError> {
        if is_single_dash(s) {
            return Ok(TrainCars::Unknown);
        }
        if let Some(n) = parse_u8_ascii(s) {
            return Ok(TrainCars::Num(n));
        }
        Err(ReMetroError::CarsConversion)
    }

    #[inline]
    pub fn sort_key(&self) -> u8 {
        match self {
            TrainCars::Num(n) => *n,
            TrainCars::Unknown => u8::MAX,
        }
    }
}

impl Ord for TrainCars {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.sort_key().cmp(&other.sort_key())
    }
}

impl PartialOrd for TrainCars {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for TrainCars {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrainCars::Num(n) => write!(f, "{n}"),
            TrainCars::Unknown => f.write_str("-"),
        }
    }
}

impl FromStr for TrainCars {
    type Err = ReMetroError;
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TrainCars::from_display_str(s)
    }
}

pub type WMATAStationCode = String;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WMATAPlatformCode(pub u8);

impl FromStr for WMATAPlatformCode {
    type Err = ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse::<u8>()?))
    }
}

impl Display for WMATAPlatformCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Represents the prediction from one train
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Train {
    pub platform: WMATAPlatformCode,
    pub line: TrainLine,
    pub cars: TrainCars,
    pub destination: MetroString<10>,
    pub min: TrainMins,
}

/// Train should sort by minutes, then by platform, then by line, then by cars, then by destination
impl Train {
    #[inline]
    pub fn sort_key(&self) -> (TrainMins, WMATAPlatformCode, TrainLine, TrainCars, MetroString<10>) {
        (self.min, self.platform, self.line, self.cars, self.destination.clone())
    }
}

impl PartialOrd for Train {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Train {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.sort_key().cmp(&other.sort_key())
    }
}

impl fmt::Display for Train {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Line: {}, Cars: {}, Destination: {}, Min: {}",
            self.line, self.cars, self.destination, self.min
        )
    }
}

/// Represents the request from the ESP32
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct PredictionsRequest {
    pub destination: MetroString<3>,
}

/// Represents the response from the web server
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct PredictionsResponse {
    pub trains: MetroVec<Train, 3>,
}
