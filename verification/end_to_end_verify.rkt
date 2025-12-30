#lang rosette

(require "model_common.rkt")

(define-symbolic base-mag real?)

(define (encoded-magnitudes)
  (define strength (strength-from-percent strength-percent))
  (for/list ([b MESSAGE-BITS])
    (* base-mag (scale-for-bit b strength))))

(define (votes-from-threshold mags threshold inverted)
  (for/list ([m mags])
    (if inverted
        (if (<= m threshold) 1.0 0.0)
        (if (>= m threshold) 1.0 0.0))))

(define (end-to-end-proof)
  (verify
   (begin
     (apply-domain-constraints)
     (assume (> base-mag 0.0))
     (assume (>= (usable-bins) (required-bins)))
     (define strength (strength-from-percent strength-percent))
     (assume (> strength 0.0))
     (assume (>= strength 0.75))

     (define mags (encoded-magnitudes))
     (define pilot (take mags (length PILOT)))
     (define sum-high (for/sum ([m pilot] [b PILOT] #:when (= b 1)) m))
     (define sum-low (for/sum ([m pilot] [b PILOT] #:when (= b 0)) m))
     (define avg-high (/ sum-high 4.0))
     (define avg-low (/ sum-low 4.0))
     (define threshold (* 0.5 (+ avg-high avg-low)))
     (assume (> (- avg-high avg-low) 1e-3))
     (define inverted (< avg-high avg-low))
     (define votes (votes-from-threshold mags threshold inverted))

     (define decoded (decide-bits mags votes threshold avg-high avg-low inverted))
     (assert (equal? decoded MESSAGE-BITS)))))

(displayln "end-to-end-proof:")
(displayln (end-to-end-proof))
