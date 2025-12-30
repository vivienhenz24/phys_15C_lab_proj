#lang rosette

(require "model_common.rkt")

(define (threshold-separation-proof)
  (verify
   (begin
     (define-symbolic high low real?)
     (score-separation-constraints high low)
     (define threshold (* 0.5 (+ high low)))
     (assert (> threshold low))
     (assert (< threshold high)))))

(define (threshold-vote-consistency-proof)
  (verify
   (begin
     (define-symbolic high low real?)
     (score-separation-constraints high low)
     (define threshold (* 0.5 (+ high low)))
     (define scores (build-pilot-scores high low))
     (define votes (for/list ([s scores]) (if (>= s threshold) 1.0 0.0)))
     (define decoded (decide-bits scores votes threshold high low #f))
     (assert (equal? decoded PILOT)))))

(displayln "decoder-threshold-separation-proof:")
(displayln (threshold-separation-proof))

(displayln "decoder-threshold-vote-consistency-proof:")
(displayln (threshold-vote-consistency-proof))
