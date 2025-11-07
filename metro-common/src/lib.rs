#![cfg_attr(not(feature = "std"), no_std)]

use thiserror::Error;

// If someone disables `std`, they must enable `heapless`.
#[cfg(all(not(feature = "std"), not(feature = "heapless")))]
compile_error!("Building `metro-common` without `std` requires enabling the `heapless` feature.");

#[cfg(feature = "std")]
pub type MetroString<const N: usize> = std::string::String;
#[cfg(feature = "std")]
pub type MetroVec<T, const N: usize> = std::vec::Vec<T>;

#[cfg(not(feature = "std"))]
pub type MetroString<const N: usize> = heapless::String<N>;
#[cfg(not(feature = "std"))]
pub type MetroVec<T, const N: usize> = heapless::Vec<T, N>;

#[derive(Debug, Error)]
pub enum ReMetroError {
    #[error("Error converting line abbreviation to strong metro type")]
    LineConversion,
    #[error("Error converting status abbreviation (e.g. ARR, BRD, DLY) to strong metro type")]
    OtherStatusConversion,
    #[error(
        "Error converting train prediction time (e.g. \"1\", \"ARR\", etc) to strong metro type"
    )]
    TrainPredictionConversion,
    #[error("Error converting number of cars to strong metro type")]
    CarsConversion,
}

pub mod predictions;
