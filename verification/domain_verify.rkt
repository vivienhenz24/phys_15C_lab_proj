#lang rosette

(require "model_common.rkt")

(define (capacity-all-proof)
  (verify
   (begin
     (apply-domain-constraints)
     (assert (>= (usable-bins) (required-bins))))))

(define (capacity-except-8k-short-proof)
  (verify
   (begin
     (apply-domain-constraints)
     (assume (not (and (= sample-rate 8000) (or (= frame-ms 20) (= frame-ms 32)))))
     (assert (>= (usable-bins) (required-bins))))))

(displayln "domain-capacity-all-proof (expected to fail for 8k/20, 8k/32):")
(displayln (capacity-all-proof))

(displayln "domain-capacity-except-8k-short-proof:")
(displayln (capacity-except-8k-short-proof))
