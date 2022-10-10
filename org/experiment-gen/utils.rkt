#lang racket/base

(require racket/string
         racket/class
         racket/draw
         pict)

(provide (all-defined-out))

(define (split-string str sep)
  (string-split str sep #:trim? #false #:repeat? #false))

;; TODO Getting a blue background with cross-hatch foreground
;; is much more difficult that I would have thought. The brush styles
;; are built-in brush-stipples, essentially bitmaps, that brush uses to
;; fill the region. Increason the line width of this hatch would
;; amount to making a custom bitmap for the fill. Unideal. @soegaard
;; suggested using MetaPict, maybe, but it doesn't look like I'd circumvent the
;; whole "making my own" bitmap of it all. The current design will remain as is
;; until I invest the time for a proper fix, this also depends on which
;; visualizations we actually go with.

;; HACK using the 'font-size as an adjuster is very crude.
;; It kind of works but you can tell it's off. Additionally
;; manually drawing the box with 'dc is hacky especially
;; since I use the same width and height as the base pict, then
;; the pen is moved further to the right.
(define (custom-rectangle rf rt cf ct base
                          #:adjust-by [adjust 0]
                          #:brush [brush (new brush%)]
                          #:pen [pen (new pen%)])
  (let* ([a adjust] [a/2 (/ adjust 2)]
         [sr (* a rf)] [er (* a (add1 rt))]
         [sc (* a/2 cf)] [ec (* a/2 (add1 ct))])
    (dc (λ (dc dx dy)
          (define old-brush (send dc get-brush))
          (define old-pen (send dc get-pen))
          (send dc set-brush brush)
          (send dc set-pen pen)
          (define path (new dc-path%))
          (send path move-to sc sr)
          (send path line-to sc er)
          (send path line-to ec er)
          (send path line-to ec sr)
          (send path close)
          (send dc draw-path path dx dy)
          (send dc set-brush old-brush)
          (send dc set-pen old-pen))
        (pict-width base) (pict-height base))))

(define (filled-triangle h base
                         #:draw-border? [db? #true]
                         #:border-width [bw 1]
                         #:border-color [bc "black"]
                         #:color [c "white"]
                         #:brush-style [bstyle 'solid])
  (dc (λ (dc dx dy)
        (define old-brush (send dc get-brush))
        (define old-pen (send dc get-pen))
        (send dc set-brush (new brush%
                                [style bstyle]
                                [color c]))
        (send dc set-pen (new pen%
                              [width (if db? bw 0)]
                              [color (if db? bc c)]))
        (define path (new dc-path%))
        (send path move-to 0 h)
        (send path line-to (/ base 2) 0)
        (send path line-to base h)
        (send path close)
        (send dc draw-path path dx dy)
        (send dc set-brush old-brush)
        (send dc set-pen old-pen))
      base h))

(define (altered-rectangle w h
                         #:draw-border? [db? #true]
                         #:border-width [bw 1]
                         #:border-color [bc "black"]
                         #:color [c "white"]
                         #:brush-style [bstyle 'solid])
  (dc (λ (dc dx dy)
        (define old-brush (send dc get-brush))
        (define old-pen (send dc get-pen))
        (send dc set-brush (new brush%
                                [style bstyle]
                                [color c]))
        (send dc set-pen (new pen%
                              [width (if db? bw 0)]
                              [color (if db? bc c)]))
        (define path (new dc-path%))

        (send path move-to 0 0)

        (send path line-to w 0)
        (send path line-to w h)
        (send path line-to 0 h)
        (send path close)
        (send dc draw-path path dx dy)
        (send dc set-brush old-brush)
        (send dc set-pen old-pen))
      w h))
