#lang rosette

(require "model_common.rkt")

(define (length-header-proof)
  (verify
   (begin
     (define len-bits (take (drop MESSAGE-BITS (length PILOT)) LENGTH-HEADER-BITS))
     (assert (= (decode-length-header len-bits) MSG-LEN)))))

(define (pilot-stats-proof)
  (verify
   (begin
     (define-symbolic high low real?)
     (score-separation-constraints high low)
     (define scores (build-pilot-scores high low))
     (define stats (frame-pilot-stats scores))
     (assert stats)
     (assert (= (list-ref stats 2) #f))
     (assert (>= (list-ref stats 1) 5)))))

(define (pilot-invert-proof)
  (verify
   (begin
     (define-symbolic high low real?)
     (score-separation-constraints low high)
     (define high* low)
     (define low* high)
     (define scores (build-pilot-scores high* low*))
     (define stats (frame-pilot-stats scores))
     (assert stats)
     (assert (= (list-ref stats 2) #t))
     (assert (>= (list-ref stats 1) 5)))))

(define (decide-bits-proof)
  (verify
   (begin
     (define-symbolic high low real?)
     (score-separation-constraints high low)
     (define avg-high high)
     (define avg-low low)
     (define threshold (* 0.5 (+ high low)))
     (define scores
       (for/list ([b MESSAGE-BITS]) (if (= b 1) high low)))
     (define votes
       (for/list ([b MESSAGE-BITS]) (if (= b 1) 1.0 0.0)))
     (define decoded (decide-bits scores votes threshold avg-high avg-low #f))
     (assert (equal? decoded MESSAGE-BITS)))))

(displayln "decoder-length-header-proof:")
(displayln (length-header-proof))

(displayln "decoder-pilot-stats-proof:")
(displayln (pilot-stats-proof))

(displayln "decoder-pilot-invert-proof:")
(displayln (pilot-invert-proof))

(displayln "decoder-decide-bits-proof:")
(displayln (decide-bits-proof))
