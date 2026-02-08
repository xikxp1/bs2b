use approx::assert_abs_diff_eq;
use bs2b::{Bs2b, Level};

const EPSILON: f64 = 1.0e-12;

// Generated from libbs2b 3.1.0 (reference C implementation) with a C harness.
// Source:
// - https://sources.debian.org/src/libbs2b/3.1.0+dfsg-2.2/src/
const GOLDEN_INPUT_F64: [f64; 32] = [
    0.0, 0.0, 0.25, -0.25, -0.75, 0.5, 1.2, -1.3, -0.1, 0.9, 0.67, -0.33, -1.0, 1.0, 0.12, -0.88,
    0.95, 0.4, -0.66, -0.44, 0.31, 0.73, -0.27, -0.58, 0.81, -0.11, -0.49, 0.62, 0.04, -0.02, 0.56,
    -0.91,
];

const GOLDEN_OUTPUT_F64_DEFAULT: [f64; 32] = [
    0.0,
    0.0,
    0.18802129058612108,
    -0.18802129058612108,
    -0.5862527579294027,
    0.3805144100134128,
    0.9190648940539746,
    -1.0,
    -0.09627798238603187,
    0.7509127536410182,
    0.5027905009367857,
    -0.21795519609654646,
    -0.7883849729625225,
    0.7970325056561118,
    0.08468808640587021,
    -0.7016767366898253,
    0.750874848669716,
    0.36008845986636945,
    -0.5529829184941018,
    -0.3378076271240977,
    0.2547831752062374,
    0.6032296231900268,
    -0.22993320217582675,
    -0.45642560472526533,
    0.6244847926866518,
    -0.04492254337064642,
    -0.3931152942332306,
    0.5108484056840342,
    0.035428214648710175,
    -0.00726956485894085,
    0.4117491056361279,
    -0.6883849292182809,
];

const GOLDEN_OUTPUT_F64_JMEIER_48K: [f64; 32] = [
    0.0,
    0.0,
    0.21166320832362484,
    -0.21166320832362484,
    -0.6480669085802713,
    0.4261706046059338,
    1.0,
    -1.0,
    -0.09759288034742608,
    0.805671095210607,
    0.5658031192176607,
    -0.2613729784146993,
    -0.869115489481349,
    0.8736995960763417,
    0.09769445539236482,
    -0.7677480682452089,
    0.8245546580581455,
    0.3737256855958568,
    -0.5929146798888964,
    -0.37567755633215416,
    0.2738316156082135,
    0.6505168511549723,
    -0.24530792334362783,
    -0.5020023550970716,
    0.6935827034589834,
    -0.07021389887134688,
    -0.4307414993445357,
    0.5520720950264516,
    0.03568046431828521,
    -0.011432319765364208,
    0.46751522625644415,
    -0.7717821370360594,
];

const GOLDEN_INPUT_I16: [i16; 24] = [
    0, 0, 1000, -1000, -20000, 15000, 32767, -32768, 12345, -23456, -30000, 29999, 77, -88, -16384,
    16383, 8192, -4096, -1, 1, 22222, -12345, -32768, 32767,
];

const GOLDEN_OUTPUT_I16_DEFAULT: [i16; 24] = [
    0, 0, 752, -752, -15272, 11157, 25502, -25557, 7905, -17102, -24456, 24283, -35, -140, -12416,
    12254, 7089, -3873, 385, -489, 17404, -9373, -25210, 25225,
];

#[test]
fn golden_vector_f64_default_level_44k1() {
    let mut dsp = Bs2b::default();
    let mut output = GOLDEN_INPUT_F64;

    dsp.process_interleaved(&mut output)
        .expect("golden input must be valid interleaved stereo");

    for (actual, expected) in output.iter().zip(GOLDEN_OUTPUT_F64_DEFAULT.iter()) {
        assert_abs_diff_eq!(*actual, *expected, epsilon = EPSILON);
    }
}

#[test]
fn golden_vector_f64_jmeier_level_48k() {
    let mut dsp = Bs2b::new(48_000, Level::JMEIER).expect("valid preset and sample rate");
    let mut output = GOLDEN_INPUT_F64;

    dsp.process_interleaved(&mut output)
        .expect("golden input must be valid interleaved stereo");

    for (actual, expected) in output.iter().zip(GOLDEN_OUTPUT_F64_JMEIER_48K.iter()) {
        assert_abs_diff_eq!(*actual, *expected, epsilon = EPSILON);
    }
}

#[test]
fn golden_vector_i16_default_level_44k1() {
    let mut dsp = Bs2b::default();
    let mut output = GOLDEN_INPUT_I16;

    dsp.process_interleaved(&mut output)
        .expect("golden input must be valid interleaved stereo");

    assert_eq!(output, GOLDEN_OUTPUT_I16_DEFAULT);
}
