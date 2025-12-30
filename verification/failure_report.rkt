#lang racket

(require "model_common.rkt")

(define (frame-len-for sr ms)
  (inexact->exact (round (/ (* sr ms) 1000.0))))

(define (next-pow2 n)
  (let loop ([p 1])
    (if (>= p n) p (loop (* p 2)))))

(define (spectrum-len-for frame-len)
  (+ (/ (next-pow2 frame-len) 2) 1))

(define (usable-bins-for spectrum-len)
  (max 0 (- spectrum-len START-BIN)))

(define (capacity-status sr ms)
  (define fl (frame-len-for sr ms))
  (define fft (next-pow2 fl))
  (define sl (spectrum-len-for fl))
  (define ub (usable-bins-for sl))
  (values fl fft sl ub (>= ub TOTAL-BITS)))

(define (report)
  (printf "Message length: ~a bytes (~a bits total)~n" MSG-LEN TOTAL-BITS)
  (printf "Strength clamp: strength = min(max(percent,15)/20, 0.6)~n")
  (printf "Start bin: ~a~n~n" START-BIN)

  (for ([sr '(8000 16000 32000)])
    (for ([ms '(20 32 64)])
      (define-values (fl fft sl ub ok?) (capacity-status sr ms))
      (printf "sr=~a ms=~a frame=~a fft=~a spectrum=~a usable=~a | payload_ok=~a"
              sr ms fl fft sl ub ok?)
      (when (not ok?)
        (printf "  <-- FAIL: not enough bins for payload"))
      (newline))))

(report)
