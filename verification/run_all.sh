#!/usr/bin/env bash
set -euo pipefail

racket verification/encoder_verify.rkt
racket verification/decoder_verify.rkt
racket verification/end_to_end_verify.rkt
racket verification/failure_catalog.rkt
