#lang rosette

;; Formal model for src/encoder.rs
;; - Real-number model (idealized), FFT/IFFT treated as identity on spectrum
;; - Verifies per-frame behavior for encode_audio_samples_with_viz
;; - Invertibility assumes equal base magnitudes for all watermark bins

(define START-BIN 48)
(define PILOT '(0 1 0 1 0 1 0 1))
(define MSG-LEN 9) ; "helloword" = 9 bytes
(define LENGTH-HEADER-BITS 16)
(define TOTAL-BITS (+ 8 LENGTH-HEADER-BITS (* 8 MSG-LEN)))
(define SPECTRUM-LEN (+ START-BIN TOTAL-BITS 1))
(define WINDOW-RADIUS 3)

(define (maxr a b) (if (>= a b) a b))
(define (minr a b) (if (<= a b) a b))

(define (bits-of-bv b size)
  (for/list ([i (in-range (- size 1) -1 -1)])
    (if (equal? (extract i i b) (bv 1 1)) 1 0)))

(define-symbolic msg0 msg1 (bitvector 8))
(define message-bytes (list msg0 msg1))

(define length-bits (bits-of-bv (bv MSG-LEN 16) 16))
(define payload-bits
  (apply append (for/list ([b message-bytes]) (bits-of-bv b 8))))
(define bits (append PILOT length-bits payload-bits))

(define-symbolic* spec-re spec-im (~> integer? real?))
(define-symbolic strength-percent real?)
(define-symbolic frame-len integer?)
(define-symbolic sample-rate integer?)
(define-symbolic frame-ms integer?)
(define-symbolic fft-len integer?)

(define (strength-from-percent sp)
  (define sp* (maxr sp 15.0))
  (minr (/ sp* 20.0) 0.6))

(define (scale-for-bit b strength)
  (if (= b 1)
      (+ 1.0 strength)
      (maxr (- 1.0 strength) 0.0)))

(define (enc-re i)
  (define strength (strength-from-percent strength-percent))
  (if (and (> frame-len START-BIN)
           (>= i START-BIN)
           (< i (+ START-BIN TOTAL-BITS)))
      (* (spec-re i) (scale-for-bit (list-ref bits (- i START-BIN)) strength))
      (spec-re i)))

(define (enc-im i)
  (define strength (strength-from-percent strength-percent))
  (if (and (> frame-len START-BIN)
           (>= i START-BIN)
           (< i (+ START-BIN TOTAL-BITS)))
      (* (spec-im i) (scale-for-bit (list-ref bits (- i START-BIN)) strength))
      (spec-im i)))

(define (mag2 re im) (+ (* re re) (* im im)))

(define (check-functional)
  (verify
   (begin
     (assume (>= frame-len 1))
     (assume (>= strength-percent 0.0))
     (for ([i (in-range SPECTRUM-LEN)])
       (define in-re (spec-re i))
       (define in-im (spec-im i))
       (define out-re (enc-re i))
       (define out-im (enc-im i))
       (define in-range? (and (>= i START-BIN) (< i (+ START-BIN TOTAL-BITS))))
       (define expected-scale
         (if (and (> frame-len START-BIN) in-range?)
             (scale-for-bit (list-ref bits (- i START-BIN))
                            (strength-from-percent strength-percent))
             1.0))
       (assert (= out-re (* in-re expected-scale)))
       (assert (= out-im (* in-im expected-scale)))))))

(define (check-invertible)
  (verify
   (begin
     (assume (> frame-len START-BIN))
     (assume (>= strength-percent 0.0))
     (define strength (strength-from-percent strength-percent))
     (assume (> strength 0.0))

     (define-symbolic base-mag-sq real?)
     (assume (> base-mag-sq 0.0))

     ;; Equal base magnitude assumption for watermark bins
     (for ([k (in-range TOTAL-BITS)])
       (define i (+ START-BIN k))
       (assert (= (mag2 (spec-re i) (spec-im i)) base-mag-sq)))

     (define (enc-mag2 i) (mag2 (enc-re i) (enc-im i)))

     (define sum0
       (for/sum ([k (in-range 8)] #:when (= (list-ref PILOT k) 0))
         (enc-mag2 (+ START-BIN k))))
     (define sum1
       (for/sum ([k (in-range 8)] #:when (= (list-ref PILOT k) 1))
         (enc-mag2 (+ START-BIN k))))

     (define avg0 (/ sum0 4.0))
     (define avg1 (/ sum1 4.0))
     (define thresh (/ (+ avg0 avg1) 2.0))

     (for ([k (in-range TOTAL-BITS)])
       (define i (+ START-BIN k))
       (define decoded (if (>= (enc-mag2 i) thresh) 1 0))
       (assert (= decoded (list-ref bits k)))))))

;; ---------------------------------------------------------------------------
;; Decoder model (simplified):
;; - Uses magnitudes directly as "scores" (monotonic proxy for log scores)
;; - Single-frame aggregation (votes are 0/1 from that frame)
;; - Windowing is not modeled; this keeps the model decidable and symbolic
;; ---------------------------------------------------------------------------

(define-symbolic base-mag real?)
(define-symbolic spectrum-len integer?)

(define (usable-bins)
  (max 0 (- spectrum-len START-BIN)))

(define (mag-at k)
  (define strength (strength-from-percent strength-percent))
  (define in-range? (and (>= k 0) (< k TOTAL-BITS)))
  (define scale (if in-range?
                    (scale-for-bit (list-ref bits k) strength)
                    1.0))
  (* base-mag scale))

(define (scores)
  (for/list ([k (in-range (usable-bins))])
    (if (< k TOTAL-BITS)
        (mag-at k)
        base-mag)))

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

(define (decide-bits scores threshold avg-high avg-low inverted)
  (define decision-band (* 0.1 (abs (- avg-high avg-low))))
  (for/list ([idx (in-range (length scores))])
    (define score (list-ref scores idx))
    (define ratio (if inverted
                      (if (<= score threshold) 1.0 0.0)
                      (if (>= score threshold) 1.0 0.0)))
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

(define (decode-bits)
  (define sc (scores))
  (define pilot-stats (frame-pilot-stats sc))
  (if (not pilot-stats)
      '() ; no valid frame
      (let* ([threshold (list-ref pilot-stats 0)]
             [matches (list-ref pilot-stats 1)]
             [inverted (list-ref pilot-stats 2)]
             [pilot (take sc (length PILOT))]
             [sum-high (for/sum ([s pilot] [b PILOT] #:when (= b 1)) s)]
             [sum-low (for/sum ([s pilot] [b PILOT] #:when (= b 0)) s)]
             [avg-high (/ sum-high 4.0)]
             [avg-low (/ sum-low 4.0)])
        (if (< matches 5)
            '()
            (decide-bits sc threshold avg-high avg-low inverted)))))

(define (check-decoder-correctness)
  (verify
   (begin
     (assume (>= frame-len 1))
     (assume (>= strength-percent 0.0))
     (assume (> base-mag 0.0))
     (assume (>= spectrum-len 0))
     (define decoded (decode-bits))
     (define have-bits? (>= (length decoded) TOTAL-BITS))
     (assert (=> have-bits?
                 (equal? (take decoded TOTAL-BITS) bits))))))

;; ---------------------------------------------------------------------------
;; Domain constraints (based on project configuration)
;; - sample_rate ∈ {8000, 16000, 32000}
;; - frame_ms ∈ {20, 32, 64}
;; - strength_percent ∈ [0, 100]
;; - frame_len derived from (sample_rate, frame_ms)
;; - fft_len derived from frame_len (next power of two)
;; - spectrum_len = fft_len/2 + 1 (RealFFT output)
;; ---------------------------------------------------------------------------

(define (in-set? x xs) (ormap (lambda (v) (= x v)) xs))

(define (apply-domain-constraints)
  (assume (in-set? sample-rate '(8000 16000 32000)))
  (assume (in-set? frame-ms '(20 32 64)))
  (assume (and (>= strength-percent 0.0) (<= strength-percent 100.0)))
  (define f1 (if (= sample-rate 8000)
                 (if (= frame-ms 20) 160 (if (= frame-ms 32) 256 512))
                 (if (= sample-rate 16000)
                     (if (= frame-ms 20) 320 (if (= frame-ms 32) 512 1024))
                     (if (= frame-ms 20) 640 (if (= frame-ms 32) 1024 2048)))))
  (assume (= frame-len f1))
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

;; ---------------------------------------------------------------------------
;; Failure classification: enumerate explicit failure reasons
;; ---------------------------------------------------------------------------

(define (fail-insufficient-bins)
  (< spectrum-len (+ START-BIN 1)))

(define (fail-insufficient-payload-bins)
  (< (usable-bins) TOTAL-BITS))

(define (fail-no-valid-pilot)
  (let ([sc (scores)])
    (or (< (length sc) (length PILOT))
        (let ([ps (frame-pilot-stats sc)])
          (or (not ps) (< (list-ref ps 1) 5))))))

(define (fail-decode-mismatch)
  (let ([decoded (decode-bits)])
    (and (>= (length decoded) TOTAL-BITS)
         (not (equal? (take decoded TOTAL-BITS) bits)))))

(define (prove-decoder-correct-under-ideal)
  (verify
   (begin
     (apply-domain-constraints)
     (assume (> base-mag 0.0))
     (assume (not (fail-insufficient-bins)))
     (assume (not (fail-insufficient-payload-bins)))
     (assume (not (fail-no-valid-pilot)))
     (define decoded (decode-bits))
     (assert (and (>= (length decoded) TOTAL-BITS)
                  (equal? (take decoded TOTAL-BITS) bits))))))

(define (find-failure predicate)
  (solve
   (begin
     (apply-domain-constraints)
     (assume (> base-mag 0.0))
     (assert (predicate)))))

(define func-result (check-functional))
(define inv-result (check-invertible))
(define dec-result (check-decoder-correctness))
(define dec-proof (prove-decoder-correct-under-ideal))
(define fail-insufficient-bins-model (find-failure fail-insufficient-bins))
(define fail-insufficient-payload-model (find-failure fail-insufficient-payload-bins))
(define fail-pilot-model (find-failure fail-no-valid-pilot))
(define fail-mismatch-model (find-failure fail-decode-mismatch))

(displayln "functional-correctness:")
(displayln func-result)
(displayln "invertibility:")
(displayln inv-result)
(displayln "decoder-correctness (simplified model):")
(displayln dec-result)
(displayln "decoder-correctness (ideal-domain proof):")
(displayln dec-proof)
(displayln "failure: insufficient bins for START_BIN:")
(displayln fail-insufficient-bins-model)
(displayln "failure: insufficient bins for full payload:")
(displayln fail-insufficient-payload-model)
(displayln "failure: pilot unusable/mismatch:")
(displayln fail-pilot-model)
(displayln "failure: decoded bits mismatch:")
(displayln fail-mismatch-model)
