/*!
 * # Safe bindings for _aubio_ library
 *
 * > _Aubio_ is a library to label music and sounds.
 * >
 * > It listens to audio signals and attempts to detect events.
 * > For instance, when a drum is hit, at which frequency is a note,
 * > or at what tempo is a rhythmic melody.
 * >
 * > Its features include segmenting a sound file before each of its attacks,
 * > performing pitch detection, tapping the beat and producing midi streams
 * > from live audio.
 * >
 * > aubio provide several algorithms and routines, including:
 * >
 * > * several onset detection methods
 * > * different pitch detection methods
 * > * tempo tracking and beat detection
 * > * MFCC (mel-frequency cepstrum coefficients)
 * > * FFT and phase vocoder
 * > * up/down-sampling
 * > * digital filters (low pass, high pass, and more)
 * > * spectral filtering
 * > * transient/steady-state separation
 * > * sound file read and write access
 * > * various mathematics utilities for music applications
 * >
 * > The name _aubio_ comes from audio with a typo: some errors are likely
 * > to be found in the results.
 *
 * ## Crate features
 *
 * The following features can be used to customize configuration:
 *
 * - _generate-bindings_ which runs __bindgen__ to generate bindings (_useful for unsupported archs_)
 */

pub(crate) use aubio_sys as ffi;

#[cfg(test)]
use aubio_lib as _;

mod fft;
mod log;
mod mfcc;
mod notes;
mod onset;
mod pitch;
mod pvoc;
mod resampler;
mod specdesc;
mod tempo;
mod types;
mod utils;
mod winfunc;
mod filterbank;

pub mod vec;

pub use self::fft::*;
pub use self::filterbank::*;
pub use self::log::*;
pub use self::mfcc::*;
pub use self::notes::*;
pub use self::onset::*;
pub use self::pitch::*;
pub use self::pvoc::*;
pub use self::resampler::*;
pub use self::specdesc::*;
pub use self::tempo::*;
pub use self::types::*;
pub use self::utils::*;
pub use self::winfunc::*;

#[macro_export]
macro_rules! farr {
    ($len: expr) => {
        [0f32; $len]
    };
}

#[macro_export]
macro_rules! carr {
    ($len: expr) => {
        [0f32; $len + 2]
    };
}
