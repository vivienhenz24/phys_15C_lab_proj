#lang racket

(define START-BIN 48)
(define PILOT-LEN 8)
(define LENGTH-HEADER-BITS 16)
(define MSG-LEN 9)
(define TOTAL-BITS (+ PILOT-LEN LENGTH-HEADER-BITS (* 8 MSG-LEN)))

(define configs
  '((8000 20) (8000 32) (8000 64)
    (16000 20) (16000 32) (16000 64)
    (32000 20) (32000 32) (32000 64)))

(define (frame-len sr ms)
  (inexact->exact (round (/ (* sr ms) 1000.0))))

(define (next-pow2 n)
  (let loop ([p 1])
    (if (>= p n) p (loop (* p 2)))))

(define (spectrum-len frame-len)
  (+ (/ (next-pow2 frame-len) 2) 1))

(define (usable-bins spectrum-len)
  (max 0 (- spectrum-len START-BIN)))

(printf "Total bits required: ~a\n" TOTAL-BITS)
(printf "\nFailure catalog (capacity + START_BIN):\n")

(for ([cfg configs])
  (define sr (first cfg))
  (define ms (second cfg))
  (define fl (frame-len sr ms))
  (define fft (next-pow2 fl))
  (define sl (spectrum-len fl))
  (define ub (usable-bins sl))
  (define start-ok? (< START-BIN sl))
  (define payload-ok? (>= ub TOTAL-BITS))
  (printf "sr=~a ms=~a frame=~a fft=~a spectrum=~a usable=~a | start_ok=~a payload_ok=~a\n"
          sr ms fl fft sl ub start-ok? payload-ok?))
