#!/usr/bin/env bash
set -euo pipefail

racket verification/encoder_verify.rkt
racket verification/decoder_verify.rkt
racket verification/decoder_threshold_verify.rkt
racket verification/bitstream_verify.rkt
racket verification/domain_verify.rkt
racket verification/end_to_end_verify.rkt
racket verification/failure_catalog.rkt
racket verification/failure_report.rkt
