# bs2b

A modern Rust implementation of the Bauer stereophonic-to-binaural (bs2b) crossfeed DSP.

This crate ports the reference bs2b algorithm into an idiomatic, type-safe API for real-time and offline audio processing.

## Features

- bs2b-compatible filter math and crossfeed behavior.
- Stateful processor suitable for streaming audio.
- Interleaved and planar stereo processing APIs.
- Supports common PCM sample formats: `f32`, `f64`, `i32`, `u32`, `i16`, `u16`, `i8`, `u8`.
- Built-in preset levels: `DEFAULT`, `CMOY`, `JMEIER`.
- Benchmarks (Criterion) and tests included.

Feature flags:

- `std` (default): enables `std::error::Error` integration for `Bs2bError`.
- `streaming`: enables optional adapters for callback/iterator streaming APIs.
- disabling default features (`default-features = false`) builds in `no_std` mode.

## Installation

```bash
cargo add bs2b
```

`no_std` mode:

```toml
[dependencies]
bs2b = { version = "0.1", default-features = false }
```

## Quick Start

```rust
use bs2b::{Bs2b, Level};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut dsp = Bs2b::new(48_000, Level::DEFAULT)?;

    // Interleaved stereo samples: L, R, L, R, ...
    let mut interleaved = vec![0.1_f32, -0.1, 0.25, -0.2, 0.3, -0.15];
    dsp.process_interleaved(&mut interleaved)?;

    Ok(())
}
```

## API Guidelines

- Use one `Bs2b` instance per stereo stream. The processor is stateful.
- Call `set_sample_rate` if stream sample rate changes. This clears internal history.
- Call `clear` after seeks, discontinuities, or stream restarts.
- Use `process_interleaved` for contiguous LR data; use `process_planar` for split channels.
- If your pipeline uses unsigned PCM, this crate applies the same signed-bias handling as the C reference implementation.

## Choosing a Level

- `Level::DEFAULT`: Closest to virtual speakers at ±30° azimuth, ~3 meters away. A moderate crossfeed.
- `Level::CMOY`: Most popular setting. Matches the parameters of Chu Moy's well-known analog crossfeed circuit.
- `Level::JMEIER`: Most subtle — makes the smallest changes to the original signal. Intended for relaxed listening. Matches Jan Meier's CORDA amplifier crossfeed.
- Custom profile:

```rust
use bs2b::Level;

let level = Level::new(700, 45)?; // cut_frequency_hz, feed_db_tenths
```

Valid ranges:

- `cut_frequency_hz`: 300..=2000
- `feed_db_tenths`: 10..=150

## Real-Time Usage Notes

- `process_frame` is available for callback-style per-frame processing.
- `streaming::CallbackAdapter` can process interleaved callback buffers (cpal-style).
- `streaming::StereoSourceAdapter` can wrap interleaved sample iterators (rodio-style).
- The library does no allocations while processing audio.
- The processor is `Clone` if you need independent state branches.

## Streaming Adapters

Enable the feature:

```toml
[dependencies]
bs2b = { version = "0.2", features = ["streaming"] }
```

cpal-style callback buffer processing:

```rust
use bs2b::{Bs2b, Level, streaming::CallbackAdapter};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let processor = Bs2b::new(48_000, Level::DEFAULT)?;
    let mut adapter = CallbackAdapter::new(processor, 2)?;
    let mut data = vec![0.0_f32; 512];

    // In your audio callback:
    // fn callback(data: &mut [f32]) {
    adapter.process(&mut data)?;
    // }
    Ok(())
}
```

rodio-style interleaved iterator processing:

```rust
use bs2b::{Bs2b, Level, streaming::StereoSourceAdapter};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let processor = Bs2b::new(48_000, Level::DEFAULT)?;
    let input = vec![0.1_f32, -0.1, 0.2, -0.2];

    let output: Vec<f32> = StereoSourceAdapter::new(input.into_iter(), processor).collect();
    let _ = output;
    Ok(())
}
```

## Testing and Benchmarks

```bash
cargo test
cargo bench
```

To regenerate C-reference golden vectors used by integration tests:

```bash
./scripts/generate_golden_vectors.sh
```

## References

- Official project: [bs2b.sourceforge.net](https://bs2b.sourceforge.net/index.html)
- Reference C implementation used for algorithm parity: [Debian source mirror](https://sources.debian.org/src/libbs2b/3.1.0+dfsg-2.2/src/)

## License

MIT
