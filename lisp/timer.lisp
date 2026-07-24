(in-package #:retrodeck)

(defconstant +ten-seconds-centisecond-nanoseconds+ 10000000)
(defconstant +ten-seconds-redraw-nanoseconds+ 16000000)
(defconstant +ten-seconds-limit+ 9999)

(defparameter *ten-seconds-cues*
  '((:start (523 28) (784 38))
    (:exact (784 35) (1047 40) (1319 55))
    (:miss (659 35) (440 55))))

(defun ten-seconds-format (centiseconds)
  (check-type centiseconds (integer 0 *))
  (let ((shown (min centiseconds +ten-seconds-limit+)))
    (format nil "~2,'0D.~2,'0D" (floor shown 100) (mod shown 100))))

(defun ten-seconds-cue-notes (cue)
  (copy-tree (cdr (or (assoc cue *ten-seconds-cues*)
                      (error "Unknown 10 Seconds cue ~S" cue)))))

(defun ten-seconds-initial-state ()
  (list :mode :ready :displayed 0 :started-at 0 :redraw-at 0))

(defun ten-seconds-elapsed (state now)
  (min +ten-seconds-limit+
       (floor (max 0 (- now (getf state :started-at)))
              +ten-seconds-centisecond-nanoseconds+)))

(defun ten-seconds-reduce (state event now)
  (check-type now (integer 0 *))
  (ecase event
    (:back (values state '((:exit))))
    (:tick
     (if (and (eq (getf state :mode) :running)
              (>= now (getf state :redraw-at)))
         (values (list :mode :running
                       :displayed (ten-seconds-elapsed state now)
                       :started-at (getf state :started-at)
                       :redraw-at (+ now +ten-seconds-redraw-nanoseconds+))
                 '((:redraw)))
         (values state nil)))
    ((:touch :controller-a)
     (if (eq (getf state :mode) :running)
         (let* ((displayed (ten-seconds-elapsed state now))
                (cue (if (= displayed 1000) :exact :miss)))
           (values (list :mode :stopped :displayed displayed
                         :started-at (getf state :started-at) :redraw-at 0)
                   (list (list :result (ten-seconds-format displayed)
                               :input event)
                         (list :cue cue) '(:redraw))))
         (values (list :mode :running :displayed 0
                       :started-at now :redraw-at now)
                 '((:cue :start) (:redraw)))))))

(defun ten-seconds-touch-event (x y)
  (check-type x integer)
  (check-type y integer)
  (if (and (<= 16 x 167) (<= 16 y 79)) :back :touch))

(defun ten-seconds-fill-source-rect (x y width height color)
  (fill-canvas-rect (+ 16 (* 2 x)) (+ 16 (* 2 y))
                    (* 2 width) (* 2 height) color))

(defun ten-seconds-draw-source-text (x y text scale color)
  (draw-text (+ 16 (* 2 x)) (+ 16 (* 2 y)) text (* 2 scale) color))

(defun ten-seconds-draw-centered-source-text (y text scale color)
  (ten-seconds-draw-source-text
   (max 0 (floor (- 624 (bitmap-text-width text scale)) 2))
   y text scale color))

(defun ten-seconds-draw-digit (x character active inactive)
  (let* ((digit (digit-char-p character))
         (mask (if digit
                   (aref #(#x3f #x06 #x5b #x4f #x66
                           #x6d #x7d #x07 #x7f #x6f) digit)
                   0)))
    (loop for (dx dy width height) in '((11 0 54 11) (65 11 11 53)
                                        (65 64 11 53) (11 117 54 11)
                                        (0 64 11 53) (0 11 11 53)
                                        (11 59 54 11))
          for index from 0
          always (ten-seconds-fill-source-rect
                  (+ x dx) (+ 43 dy) width height
                  (if (logbitp index mask) active inactive)))))

(defun render-ten-seconds (state)
  (let* ((mode (getf state :mode))
         (displayed (getf state :displayed))
         (background #x100d0c) (amber #xff7138) (dim-amber #x1c1c1c)
         (cream #xffedc2) (muted #xaa8f7c) (success #x62d38c)
         (digit-color (if (and (eq mode :stopped) (= displayed 1000))
                          success amber))
         (shown (ten-seconds-format displayed)))
    (check-type displayed (integer 0 9999))
    (unless (member mode '(:ready :running :stopped))
      (error "Unknown 10 Seconds mode ~S" mode))
    (and (clear-canvas #x000000)
         (ten-seconds-fill-source-rect 0 0 624 224 background)
         (ten-seconds-fill-source-rect 6 5 70 25 #x29211e)
         (ten-seconds-draw-source-text 15 11 "BACK" 2 cream)
         (ten-seconds-draw-centered-source-text 9 "STOP AT 10.00" 2 cream)
         (loop for position across #(129 219 329 419)
               for index across #(0 1 3 4)
               always (ten-seconds-draw-digit position (char shown index)
                                              digit-color dim-amber))
         (ten-seconds-fill-source-rect 303 149 14 14 digit-color)
         (or (not (eq mode :stopped))
             (ten-seconds-draw-centered-source-text
              178 (cond ((= displayed 1000) "EXACT")
                        ((< displayed 1000)
                         (format nil "~A EARLY"
                                 (ten-seconds-format (- 1000 displayed))))
                        (t (format nil "~A LATE"
                                   (ten-seconds-format (- displayed 1000)))))
              1 (if (= displayed 1000) success muted)))
         (ten-seconds-draw-centered-source-text
          198 (ecase mode
                (:ready "TAP OR A TO START")
                (:running "TAP OR A TO STOP")
                (:stopped "TAP OR A FOR ANOTHER TRY"))
          2 cream))))
