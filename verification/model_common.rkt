#lang rosette

(provide
 START-BIN PILOT LENGTH-HEADER-BITS MSG-LEN TOTAL-BITS
 MESSAGE-BITS message-bytes bits-of-bv
 strength-from-percent scale-for-bit
 score-separation-constraints
 sample-rate frame-ms frame-len fft-len spectrum-len strength-percent
 apply-domain-constraints
 usable-bins required-bins
 encoder-scale-re encoder-scale-im
 build-pilot-scores frame-pilot-stats
 decide-bits decode-length-header
)

(define START-BIN 48)
(define PILOT '(0 1 0 1 0 1 0 1))
(define LENGTH-HEADER-BITS 16)
(define MSG-LEN 9) ; "helloword"
(define TOTAL-BITS (+ 8 LENGTH-HEADER-BITS (* 8 MSG-LEN)))

(define (maxr a b) (if (>= a b) a b))
(define (minr a b) (if (<= a b) a b))

(define (bits-of-bv b size)
  (for/list ([i (in-range (- size 1) -1 -1)])
    (if (equal? (extract i i b) (bv 1 1)) 1 0)))

(define (string->bv-bytes s)
  (for/list ([b (bytes->list (string->bytes/utf-8 s))])
    (bv b 8)))

(define message-bytes (string->bv-bytes "helloword"))

(define (build-message-bits)
  (define length-bits (bits-of-bv (bv MSG-LEN 16) 16))
  (define payload-bits
    (apply append (for/list ([b message-bytes]) (bits-of-bv b 8))))
  (append PILOT length-bits payload-bits))

(define MESSAGE-BITS (build-message-bits))

(define-symbolic strength-percent real?)
(define-symbolic sample-rate integer?)
(define-symbolic frame-ms integer?)
(define-symbolic frame-len integer?)
(define-symbolic fft-len integer?)
(define-symbolic spectrum-len integer?)

(define (strength-from-percent sp)
  (define sp* (maxr sp 15.0))
  (minr (/ sp* 20.0) 0.6))

(define (scale-for-bit b strength)
  (if (= b 1)
      (+ 1.0 strength)
      (maxr (- 1.0 strength) 0.0)))

(define (in-set? x xs) (ormap (lambda (v) (= x v)) xs))

(define (apply-domain-constraints)
  (assume (in-set? sample-rate '(8000 16000 32000)))
  (assume (in-set? frame-ms '(20 32 64)))
  (assume (and (>= strength-percent 15.0) (<= strength-percent 100.0)))
  ;; frame_len = round(sample_rate * frame_ms / 1000)
  (define f1 (if (= sample-rate 8000)
                 (if (= frame-ms 20) 160 (if (= frame-ms 32) 256 512))
                 (if (= sample-rate 16000)
                     (if (= frame-ms 20) 320 (if (= frame-ms 32) 512 1024))
                     (if (= frame-ms 20) 640 (if (= frame-ms 32) 1024 2048)))))
  (assume (= frame-len f1))
  ;; next power of two
  (define f2 (cond
               [(<= frame-len 2) 2]
               [(<= frame-len 4) 4]
               [(<= frame-len 8) 8]
               [(<= frame-len 16) 16]
               [(<= frame-len 32) 32]
               [(<= frame-len 64) 64]
               [(<= frame-len 128) 128]
               [(<= frame-len 256) 256]
               [(<= frame-len 512) 512]
               [(<= frame-len 1024) 1024]
               [(<= frame-len 2048) 2048]
               [else 4096]))
  (assume (= fft-len f2))
  (assume (= spectrum-len (+ (/ fft-len 2) 1))))

(define (usable-bins)
  (max 0 (- spectrum-len START-BIN)))

(define (required-bins)
  TOTAL-BITS)

(define (encoder-scale-re spec-re i)
  (define strength (strength-from-percent strength-percent))
  (define in-range? (and (>= i START-BIN)
                         (< i (+ START-BIN TOTAL-BITS))))
  (define scale
    (if (and (< START-BIN spectrum-len) in-range?)
        (scale-for-bit (list-ref MESSAGE-BITS (- i START-BIN)) strength)
        1.0))
  (* (spec-re i) scale))

(define (encoder-scale-im spec-im i)
  (define strength (strength-from-percent strength-percent))
  (define in-range? (and (>= i START-BIN)
                         (< i (+ START-BIN TOTAL-BITS))))
  (define scale
    (if (and (< START-BIN spectrum-len) in-range?)
        (scale-for-bit (list-ref MESSAGE-BITS (- i START-BIN)) strength)
        1.0))
  (* (spec-im i) scale))

;; Decoder helpers (idealized score model)

(define (build-pilot-scores high low)
  (for/list ([b PILOT])
    (if (= b 1) high low)))

(define (frame-pilot-stats scores)
  (if (< (length scores) (length PILOT))
      #f
      (let* ([pilot (take scores (length PILOT))]
             [sum-high (for/sum ([s pilot] [b PILOT] #:when (= b 1)) s)]
             [sum-low (for/sum ([s pilot] [b PILOT] #:when (= b 0)) s)]
             [count-high (length (filter (lambda (b) (= b 1)) PILOT))]
             [count-low (length (filter (lambda (b) (= b 0)) PILOT))]
             [threshold (* 0.5 (+ (/ sum-high count-high) (/ sum-low count-low)))]
             [matches-normal (for/sum ([s pilot] [b PILOT])
                               (if (= (if (>= s threshold) 1 0) b) 1 0))]
             [matches-inverted (for/sum ([s pilot] [b PILOT])
                                 (if (= (if (<= s threshold) 1 0) b) 1 0))])
        (if (> matches-inverted matches-normal)
            (list threshold matches-inverted #t)
            (list threshold matches-normal #f)))))

(define (score-separation-constraints high low)
  ;; Ensure strict separation to avoid threshold ambiguity.
  (assume (> high low))
  (assume (> (- high low) 1e-3)))

(define (decide-bits scores votes threshold avg-high avg-low inverted)
  (define decision-band (* 0.1 (abs (- avg-high avg-low))))
  (for/list ([idx (in-range (length scores))])
    (define score (list-ref scores idx))
    (define ratio (list-ref votes idx))
    (define effective-ratio (if inverted (- 1.0 ratio) ratio))
    (define-values (bit-is-one bit-is-zero soft-cmp)
      (if inverted
          (values (<= score threshold)
                  (>= score (+ threshold (* decision-band 3.0)))
                  (<= score (+ threshold (* decision-band 0.75))))
          (values (>= score threshold)
                  (<= score (- threshold (* decision-band 3.0)))
                  (>= score (- threshold (* decision-band 0.75))))))
    (define in-length-header
      (and (>= idx (length PILOT))
           (< idx (+ (length PILOT) LENGTH-HEADER-BITS))))
    (cond
      [in-length-header (if (and (>= effective-ratio 0.54) bit-is-one) 1 0)]
      [bit-is-one 1]
      [bit-is-zero 0]
      [else (if (or (>= effective-ratio 0.45) soft-cmp) 1 0)])))

(define (decode-length-header bits)
  (define len 0)
  (for ([bit bits])
    (set! len (+ (* len 2) (if (= bit 1) 1 0))))
  len)
