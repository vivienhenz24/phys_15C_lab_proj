#lang rosette

(require "model_common.rkt")

(define (bitstream-length-proof)
  (verify
   (begin
     (assert (= (length MESSAGE-BITS) TOTAL-BITS))
     (assert (equal? (take MESSAGE-BITS (length PILOT)) PILOT))
     (define len-bits (take (drop MESSAGE-BITS (length PILOT)) LENGTH-HEADER-BITS))
     (assert (= (decode-length-header len-bits) MSG-LEN)))))

(define (payload-bits-proof)
  (verify
   (begin
     (define payload (drop MESSAGE-BITS (+ (length PILOT) LENGTH-HEADER-BITS)))
     (define expected
       (apply append (for/list ([b message-bytes]) (bits-of-bv b 8))))
     (assert (equal? payload expected)))))

(displayln "bitstream-length-proof:")
(displayln (bitstream-length-proof))

(displayln "payload-bits-proof:")
(displayln (payload-bits-proof))
