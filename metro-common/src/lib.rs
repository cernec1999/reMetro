#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(all(feature = "std", not(feature = "no_std")))]
pub type MetroString<const N: usize> = std::string::String;
#[cfg(all(feature = "std", not(feature = "no_std")))]
pub type MetroVec<T, const N: usize> = std::vec::Vec<T>;

#[cfg(any(not(feature = "std"), feature = "no_std"))]
pub type MetroString<const N: usize> = heapless::String<N>;
#[cfg(any(not(feature = "std"), feature = "no_std"))]
pub type MetroVec<T, const N: usize> = heapless::Vec<T, N>;

pub mod predictions;