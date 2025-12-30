#lang rosette

(require "model_common.rkt")

(define-symbolic* spec-re spec-im (~> integer? real?))

(define (encoder-functional-proof)
  (verify
   (begin
     (apply-domain-constraints)
     (define strength (strength-from-percent strength-percent))
     (assume (>= strength 0.0))
     (for ([i (in-range (+ START-BIN TOTAL-BITS))])
       (define in-re (spec-re i))
       (define in-im (spec-im i))
       (define out-re (encoder-scale-re spec-re i))
       (define out-im (encoder-scale-im spec-im i))
       (define in-range? (and (>= i START-BIN)
                              (< i (+ START-BIN TOTAL-BITS))
                              (< START-BIN spectrum-len)))
       (define expected-scale
         (if in-range?
             (scale-for-bit (list-ref MESSAGE-BITS (- i START-BIN)) strength)
             1.0))
       (assert (= out-re (* in-re expected-scale)))
       (assert (= out-im (* in-im expected-scale)))))))

(define (encoder-capacity-proof)
  (verify
   (begin
     (apply-domain-constraints)
     (assert (>= (usable-bins) (required-bins))))))

(define (strength-floor-proof)
  (verify
   (begin
     (apply-domain-constraints)
     (define strength (strength-from-percent strength-percent))
     (assert (= strength 0.6)))))

(displayln "encoder-functional-proof:")
(displayln (encoder-functional-proof))

(displayln "encoder-capacity-proof:")
(displayln (encoder-capacity-proof))

(displayln "encoder-strength-floor-proof:")
(displayln (strength-floor-proof))
