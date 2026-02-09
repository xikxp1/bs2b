# bs2b Architecture

## 1. Goals

This implementation targets three constraints simultaneously:

1. Preserve the original bs2b algorithm behavior.
2. Expose a practical Rust API for real-world audio pipelines.
3. Keep processing fast, allocation-free, and stateful.

## 2. High-Level Design

The library centers around a single stateful processor:

- `Bs2b`: owns configuration, filter coefficients, and runtime filter state.

Public-facing supporting types:

- `Level`: validated crossfeed profile (`cut_frequency_hz`, `feed_db_tenths`).
- `Sample`: sealed trait for supported sample formats.
- `Bs2bError`: typed configuration and buffer-shape errors.

Internal-only types:

- `Coefficients`: precomputed filter parameters.
- `FilterState`: per-channel history for IIR recurrence.

## 3. Signal Path

For each stereo frame `(L, R)`:

1. Apply low-pass IIR to each channel.
2. Apply high-boost IIR to each channel.
3. Crossfeed low-pass output from opposite channel.
4. Apply global gain compensation.
5. Clip to target sample-domain limits.

Mathematically (same structure as reference C code):

- Low-pass: `lo[n] = a0_lo * x[n] + b1_lo * lo[n-1]`
- High-boost: `hi[n] = a0_hi * x[n] + a1_hi * x[n-1] + b1_hi * hi[n-1]`
- Crossfeed output:
  - `L' = (hi_L + lo_R) * gain`
  - `R' = (hi_R + lo_L) * gain`

## 4. Coefficient Generation

`Coefficients::from_level` translates `Level + sample_rate` into DSP coefficients.

Derived intermediate values:

- `GB_lo`, `GB_hi`: gain terms in dB
- `G_lo`, `G_hi`: linear gain terms
- `Fc_hi`: high-boost cutoff

Recurrence coefficients follow the original equations:

- `x = exp(-2*pi*Fc / sample_rate)`
- low-pass: `b1_lo = x`, `a0_lo = G_lo * (1 - x)`
- high-boost: `b1_hi = x`, `a0_hi = 1 - G_hi * (1 - x)`, `a1_hi = -x`
- `gain = 1 / (1 - G_hi + G_lo)`

## 5. State Model

`FilterState` keeps three 2-channel histories:

- `asis`: previous unfiltered input (`x[n-1]`)
- `lo`: previous low-pass output
- `hi`: previous high-boost output

State behavior:

- Preserved when changing level (`set_level`) to match the historical bs2b style.
- Cleared on sample-rate change (`set_sample_rate`) because recurrence coefficients change.
- Can be manually reset (`clear`) for stream discontinuities.

## 6. Sample Conversion Strategy

The processor executes in `f64` internally.

`Sample` implementations map each external type to a signed processing domain:

- Floating point: `[-1.0, 1.0]`
- Signed integers: native signed range
- Unsigned integers: XOR bias conversion (same strategy as C implementation)

After processing, results are clipped to type domain limits and converted back.

## 7. API Surface and Ergonomics

Primary entry points:

- `process_frame`: callback-style frame processing.
- `process_interleaved`: in-place `L,R,L,R` buffer processing.
- `process_planar`: in-place left/right buffer processing.
- `streaming::CallbackAdapter`: callback-buffer adapter for cpal-style APIs.
- `streaming::StereoSourceAdapter`: iterator adapter for rodio-style APIs.

Validation decisions:

- Invalid sample rate and invalid level values are explicit errors.
- Interleaved odd-length and planar length mismatch are explicit errors.

## 8. Performance Notes

- No heap allocation during processing.
- Tight recurrence loop over mutable slices.
- Criterion benchmarks cover both interleaved and planar paths.

Potential future extension points:

- SIMD-accelerated batch kernels.

## 9. Testing Strategy

The test suite verifies:

- Configuration validation.
- Delay formula behavior.
- Clear/reset semantics.
- Buffer shape error handling.
- Planar/interleaved parity.
- Clipping behavior.
- Signed/unsigned conversion parity.
- Deterministic DSP output against an independent recurrence path.
- Golden-vector parity against libbs2b C outputs (`tests/golden_vectors.rs`).

## 10. Reference Mapping

Algorithm and constants were implemented from:

- bs2b project documentation: <https://bs2b.sourceforge.net/index.html>
- reference C code (`bs2b.c`, `bs2b.h`): <https://sources.debian.org/src/libbs2b/3.1.0+dfsg-2.2/src/>
