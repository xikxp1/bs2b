//! Streaming adapters for callback-oriented audio pipelines.
//!
//! These adapters are useful for APIs that deliver interleaved buffers
//! (for example, `cpal` output callbacks) or sample iterators
//! (for example, `rodio::Source`-style iterators).

use core::fmt;
use core::iter::FusedIterator;

use crate::{Bs2b, Sample};

/// Errors returned by streaming adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamingError {
    InvalidChannelCount(usize),
    InvalidChannelPair {
        channels: usize,
        left: usize,
        right: usize,
    },
    MisalignedCallbackBuffer {
        sample_count: usize,
        channels: usize,
    },
}

impl fmt::Display for StreamingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidChannelCount(channels) => write!(
                f,
                "channel count must be at least 2 for stereo crossfeed, got {channels}"
            ),
            Self::InvalidChannelPair {
                channels,
                left,
                right,
            } => write!(
                f,
                "channel pair ({left}, {right}) is invalid for buffer with {channels} channels"
            ),
            Self::MisalignedCallbackBuffer {
                sample_count,
                channels,
            } => write!(
                f,
                "callback buffer has {sample_count} samples, which is not divisible by channel count {channels}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for StreamingError {}

/// In-place adapter for callback APIs that provide interleaved frame buffers.
///
/// The adapter processes exactly one stereo channel pair per frame, and leaves
/// all other channels untouched.
#[derive(Debug, Clone)]
pub struct CallbackAdapter {
    processor: Bs2b,
    channels: usize,
    left_channel: usize,
    right_channel: usize,
}

impl CallbackAdapter {
    /// Creates an adapter that processes channel pair `(0, 1)`.
    pub fn new(processor: Bs2b, channels: usize) -> Result<Self, StreamingError> {
        Self::with_channel_pair(processor, channels, 0, 1)
    }

    /// Creates an adapter that processes the selected stereo channel pair.
    pub fn with_channel_pair(
        processor: Bs2b,
        channels: usize,
        left_channel: usize,
        right_channel: usize,
    ) -> Result<Self, StreamingError> {
        validate_channel_pair(channels, left_channel, right_channel)?;

        Ok(Self {
            processor,
            channels,
            left_channel,
            right_channel,
        })
    }

    /// Returns immutable access to the wrapped processor.
    pub const fn processor(&self) -> &Bs2b {
        &self.processor
    }

    /// Returns mutable access to the wrapped processor.
    pub fn processor_mut(&mut self) -> &mut Bs2b {
        &mut self.processor
    }

    /// Consumes the adapter and returns the wrapped processor.
    pub fn into_processor(self) -> Bs2b {
        self.processor
    }

    /// Returns total channel count expected in callback buffers.
    pub const fn channels(&self) -> usize {
        self.channels
    }

    /// Returns left channel index used for crossfeed.
    pub const fn left_channel(&self) -> usize {
        self.left_channel
    }

    /// Returns right channel index used for crossfeed.
    pub const fn right_channel(&self) -> usize {
        self.right_channel
    }

    /// Processes one callback buffer in-place.
    pub fn process<T: Sample>(&mut self, data: &mut [T]) -> Result<(), StreamingError> {
        if !data.len().is_multiple_of(self.channels) {
            return Err(StreamingError::MisalignedCallbackBuffer {
                sample_count: data.len(),
                channels: self.channels,
            });
        }

        for frame in data.chunks_exact_mut(self.channels) {
            let (left, right) = self
                .processor
                .process_frame(frame[self.left_channel], frame[self.right_channel]);
            frame[self.left_channel] = left;
            frame[self.right_channel] = right;
        }

        Ok(())
    }
}

/// Iterator adapter for stereo interleaved sample streams.
///
/// The wrapped iterator is read in `(left, right)` pairs. If the iterator ends
/// with a single trailing sample, that sample is passed through unchanged.
#[derive(Debug, Clone)]
pub struct StereoSourceAdapter<I>
where
    I: Iterator,
{
    inner: I,
    processor: Bs2b,
    pending: Option<I::Item>,
}

impl<I> StereoSourceAdapter<I>
where
    I: Iterator,
    I::Item: Sample,
{
    /// Creates a new stereo source adapter.
    pub fn new(inner: I, processor: Bs2b) -> Self {
        Self {
            inner,
            processor,
            pending: None,
        }
    }

    /// Returns immutable access to the wrapped processor.
    pub const fn processor(&self) -> &Bs2b {
        &self.processor
    }

    /// Returns mutable access to the wrapped processor.
    pub fn processor_mut(&mut self) -> &mut Bs2b {
        &mut self.processor
    }

    /// Consumes the adapter and returns the wrapped iterator.
    ///
    /// If called while one processed sample is buffered, that buffered sample is dropped.
    pub fn into_inner(self) -> I {
        self.inner
    }
}

impl<I> Iterator for StereoSourceAdapter<I>
where
    I: Iterator,
    I::Item: Sample,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(sample) = self.pending.take() {
            return Some(sample);
        }

        let left = self.inner.next()?;
        let Some(right) = self.inner.next() else {
            return Some(left);
        };

        let (left, right) = self.processor.process_frame(left, right);
        self.pending = Some(right);
        Some(left)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (low, high) = self.inner.size_hint();
        let extra = usize::from(self.pending.is_some());
        (
            low.saturating_add(extra),
            high.and_then(|value| value.checked_add(extra)),
        )
    }
}

impl<I> FusedIterator for StereoSourceAdapter<I>
where
    I: FusedIterator,
    I::Item: Sample,
{
}

fn validate_channel_pair(
    channels: usize,
    left_channel: usize,
    right_channel: usize,
) -> Result<(), StreamingError> {
    if channels < 2 {
        return Err(StreamingError::InvalidChannelCount(channels));
    }

    if left_channel >= channels || right_channel >= channels || left_channel == right_channel {
        return Err(StreamingError::InvalidChannelPair {
            channels,
            left: left_channel,
            right: right_channel,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;
    use crate::Level;

    #[test]
    fn callback_adapter_matches_stereo_interleaved_processing() {
        let mut expected = vec![0.2_f32, -0.2, 0.35, -0.15, -0.4, 0.25];
        let mut actual = expected.clone();

        let mut direct = Bs2b::new(48_000, Level::DEFAULT).expect("valid config");
        direct
            .process_interleaved(&mut expected)
            .expect("buffer is valid stereo");

        let processor = Bs2b::new(48_000, Level::DEFAULT).expect("valid config");
        let mut adapter = CallbackAdapter::new(processor, 2).expect("valid adapter config");
        adapter
            .process(&mut actual)
            .expect("buffer alignment is valid");

        assert_eq!(actual, expected);
    }

    #[test]
    fn callback_adapter_processes_only_selected_pair() {
        let mut data = vec![
            0.7_f32, -0.7, 0.2, -0.2, //
            0.6, -0.6, 0.4, -0.1, //
            0.5, -0.5, -0.3, 0.3,
        ];

        let mut expected_pair = vec![0.2_f32, -0.2, 0.4, -0.1, -0.3, 0.3];
        let mut direct = Bs2b::new(48_000, Level::DEFAULT).expect("valid config");
        direct
            .process_interleaved(&mut expected_pair)
            .expect("buffer is valid stereo");

        let processor = Bs2b::new(48_000, Level::DEFAULT).expect("valid config");
        let mut adapter =
            CallbackAdapter::with_channel_pair(processor, 4, 2, 3).expect("valid pair");
        adapter
            .process(&mut data)
            .expect("buffer alignment is valid");

        assert_eq!(data[0], 0.7_f32);
        assert_eq!(data[1], -0.7_f32);
        assert_eq!(data[4], 0.6_f32);
        assert_eq!(data[5], -0.6_f32);
        assert_eq!(data[8], 0.5_f32);
        assert_eq!(data[9], -0.5_f32);

        assert_eq!(data[2], expected_pair[0]);
        assert_eq!(data[3], expected_pair[1]);
        assert_eq!(data[6], expected_pair[2]);
        assert_eq!(data[7], expected_pair[3]);
        assert_eq!(data[10], expected_pair[4]);
        assert_eq!(data[11], expected_pair[5]);
    }

    #[test]
    fn callback_adapter_validates_channels_and_buffer_shape() {
        let processor = Bs2b::new(48_000, Level::DEFAULT).expect("valid config");
        assert!(matches!(
            CallbackAdapter::new(processor.clone(), 1),
            Err(StreamingError::InvalidChannelCount(1))
        ));
        assert!(matches!(
            CallbackAdapter::with_channel_pair(processor.clone(), 2, 0, 0),
            Err(StreamingError::InvalidChannelPair {
                channels: 2,
                left: 0,
                right: 0
            })
        ));

        let mut adapter = CallbackAdapter::new(processor, 2).expect("valid adapter config");
        let mut misaligned = vec![0.1_f32, -0.2, 0.3];
        assert!(matches!(
            adapter.process(&mut misaligned),
            Err(StreamingError::MisalignedCallbackBuffer {
                sample_count: 3,
                channels: 2
            })
        ));
    }

    #[test]
    fn stereo_source_adapter_matches_interleaved_processing() {
        let input = vec![0.1_f32, -0.1, 0.2, -0.3, 0.4, 0.05];

        let mut expected = input.clone();
        let mut direct = Bs2b::new(48_000, Level::DEFAULT).expect("valid config");
        direct
            .process_interleaved(&mut expected)
            .expect("buffer is valid stereo");

        let processor = Bs2b::new(48_000, Level::DEFAULT).expect("valid config");
        let actual: vec::Vec<f32> =
            StereoSourceAdapter::new(input.into_iter(), processor).collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn stereo_source_adapter_passes_through_odd_tail_sample() {
        let input = vec![0.3_f32, -0.25, 0.5];

        let mut expected = vec![input[0], input[1]];
        let mut direct = Bs2b::new(48_000, Level::DEFAULT).expect("valid config");
        direct
            .process_interleaved(&mut expected)
            .expect("buffer is valid stereo");
        expected.push(input[2]);

        let processor = Bs2b::new(48_000, Level::DEFAULT).expect("valid config");
        let actual: vec::Vec<f32> =
            StereoSourceAdapter::new(input.into_iter(), processor).collect();
        assert_eq!(actual, expected);
    }
}
