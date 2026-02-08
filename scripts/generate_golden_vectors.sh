#!/usr/bin/env bash
set -euo pipefail

# Regenerates golden vectors from libbs2b 3.1.0 reference C implementation.
# Output is printed to stdout in Rust constant format.

work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

curl -fsSL 'https://sources.debian.org/data/main/libb/libbs2b/3.1.0%2Bdfsg-2.2/src/bs2b.c' -o "$work_dir/bs2b.c"
curl -fsSL 'https://sources.debian.org/data/main/libb/libbs2b/3.1.0%2Bdfsg-2.2/src/bs2b.h' -o "$work_dir/bs2b.h"
curl -fsSL 'https://sources.debian.org/data/main/libb/libbs2b/3.1.0%2Bdfsg-2.2/src/bs2btypes.h' -o "$work_dir/bs2btypes.h"
curl -fsSL 'https://sources.debian.org/data/main/libb/libbs2b/3.1.0%2Bdfsg-2.2/src/bs2bversion.h' -o "$work_dir/bs2bversion.h"

cat > "$work_dir/gen.c" <<'C_EOF'
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include "bs2b.h"

static void print_f64(const char *name, const double *data, int len) {
    printf("const %s: [f64; %d] = [\n", name, len);
    for (int i = 0; i < len; ++i) {
        printf("    %.17g,\n", data[i]);
    }
    printf("];\n\n");
}

static void print_i16(const char *name, const int16_t *data, int len) {
    printf("const %s: [i16; %d] = [\n", name, len);
    for (int i = 0; i < len; ++i) {
        printf("    %d,\n", data[i]);
    }
    printf("];\n\n");
}

int main(void) {
    const double input_f64[] = {
        0.0, 0.0, 0.25, -0.25, -0.75, 0.5, 1.2, -1.3,
        -0.1, 0.9, 0.67, -0.33, -1.0, 1.0, 0.12, -0.88,
        0.95, 0.4, -0.66, -0.44, 0.31, 0.73, -0.27, -0.58,
        0.81, -0.11, -0.49, 0.62, 0.04, -0.02, 0.56, -0.91,
    };

    const int input_len = (int)(sizeof(input_f64) / sizeof(input_f64[0]));
    const int frames = input_len / 2;

    double out_default[input_len];
    double out_jmeier_48k[input_len];
    memcpy(out_default, input_f64, sizeof(input_f64));
    memcpy(out_jmeier_48k, input_f64, sizeof(input_f64));

    t_bs2bdp a = bs2b_open();
    bs2b_cross_feed_d(a, out_default, frames);

    t_bs2bdp b = bs2b_open();
    bs2b_set_srate(b, 48000);
    bs2b_set_level(b, BS2B_JMEIER_CLEVEL);
    bs2b_cross_feed_d(b, out_jmeier_48k, frames);

    const int16_t input_i16[] = {
        0, 0, 1000, -1000, -20000, 15000, 32767, -32768,
        12345, -23456, -30000, 29999, 77, -88, -16384, 16383,
        8192, -4096, -1, 1, 22222, -12345, -32768, 32767,
    };

    const int i16_len = (int)(sizeof(input_i16) / sizeof(input_i16[0]));
    int16_t out_i16_default[i16_len];
    memcpy(out_i16_default, input_i16, sizeof(input_i16));

    t_bs2bdp c = bs2b_open();
    bs2b_cross_feed_s16(c, out_i16_default, i16_len / 2);

    print_f64("GOLDEN_INPUT_F64", input_f64, input_len);
    print_f64("GOLDEN_OUTPUT_F64_DEFAULT", out_default, input_len);
    print_f64("GOLDEN_OUTPUT_F64_JMEIER_48K", out_jmeier_48k, input_len);
    print_i16("GOLDEN_INPUT_I16", input_i16, i16_len);
    print_i16("GOLDEN_OUTPUT_I16_DEFAULT", out_i16_default, i16_len);

    bs2b_close(a);
    bs2b_close(b);
    bs2b_close(c);
    return 0;
}
C_EOF

cc -std=c99 -O2 "$work_dir/gen.c" "$work_dir/bs2b.c" -lm -o "$work_dir/gen"
"$work_dir/gen"
