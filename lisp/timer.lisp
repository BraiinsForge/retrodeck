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

(defun ten-seconds-runtime-volume (&optional (text nil supplied-p))
  (handler-case
      (if supplied-p (parse-dashboard-inherited-volume text)
          (dashboard-inherited-volume))
    (error ()
      (format *error-output*
              "ten-seconds-deck: volume must be an integer from 0 through 100; game cues disabled~%")
      (finish-output *error-output*)
      0)))

(defun ten-seconds-wayland-requested-p (presentation display)
  (check-type presentation (or null string))
  (check-type display (or null string))
  (and (stringp presentation) (string= presentation "layer-shell")
       (stringp display) (plusp (length display))))

(defun make-ten-seconds-runtime
    (&key
       (presentation (dashboard-environment-value "RETRO_DECK_PRESENTATION"))
       (wayland-display
         (dashboard-environment-value *dashboard-wayland-display-environment*))
       (clock #'monotonic-nanoseconds))
  (check-type clock function)
  (let* ((wayland (ten-seconds-wayland-requested-p presentation wayland-display))
         (runtime (make-dashboard-runtime :wayland wayland
                                          :wayland-display wayland-display
                                          :default-volume 42 :clock clock)))
    (setf (getf runtime :volume) nil
          (getf runtime :dirty) t)
    runtime))

(defmacro with-ten-seconds-cleanup ((cleanup) &body body)
  `(let ((failure nil) (results nil))
     (unwind-protect
         (handler-case (setf results (multiple-value-list (progn ,@body)))
           (error (condition) (setf failure condition)))
       (handler-case ,cleanup
         (error (condition) (unless failure (setf failure condition)))))
     (if failure (error failure) (values-list results))))

(defun ten-seconds-runtime-shutdown (runtime)
  (unwind-protect
      (dashboard-runtime-shutdown runtime #'stop-audio)
    (setf (getf runtime :dirty) nil))
  runtime)

(defun ten-seconds-runtime-present (runtime)
  (when (and (getf runtime :wayland)
             (not (getf runtime :presentation-owned-p)))
    (unless (open-wayland-gameplay-at (getf runtime :wayland-display))
      (return-from ten-seconds-runtime-present nil))
    (setf (getf runtime :presentation-owned-p) t))
  (dashboard-runtime-present runtime))

(defun ten-seconds-runtime-initialize (runtime)
  (check-type runtime list)
  (when (getf runtime :initialized-p)
    (error "10 Seconds runtime is already initialized"))
  (let ((completed nil))
    (with-ten-seconds-cleanup
        ((unless completed (ten-seconds-runtime-shutdown runtime)))
      (unless (getf runtime :wayland)
        (unless (open-fbdev)
          (error "10 Seconds presentation did not open"))
        (setf (getf runtime :presentation-owned-p) t))
      (unless (open-evdev-touch)
        (error "10 Seconds touchscreen did not open"))
      (setf (getf runtime :touch-owned-p) t
            (getf runtime :controls-owned-p) t)
      (multiple-value-bind (gamepads error) (scan-evdev-gamepads)
        (when error
          (format *error-output*
                  "ten-seconds-deck: controller input unavailable: ~A~%"
                  error))
        (format *error-output*
                "ten-seconds-deck: ~D THEGamepad controller(s) ready; physical A starts and stops the timer~%"
                gamepads)
        (finish-output *error-output*))
      (setf (getf runtime :volume) (ten-seconds-runtime-volume)
            (getf runtime :dirty) t
            (getf runtime :initialized-p) t
            (getf runtime :running) t
            completed t)
      runtime)))

(defun ten-seconds-runtime-run-iteration (state runtime)
  (unless (and (getf runtime :initialized-p) (getf runtime :running))
    (error "10 Seconds runtime is not running"))
  (when (= (process-shutdown-p) 1)
    (setf (getf runtime :running) nil)
    (return-from ten-seconds-runtime-run-iteration
      (values state runtime '((:shutdown)))))
  (let ((current state) (tick-now 0) (trace nil))
    (labels ((record (item) (push item trace))
             (effect (item)
               (case (first item)
                 (:exit (setf (getf runtime :running) nil))
                 (:result
                  (format *error-output* "ten-seconds-deck: result=~A input=~(~A~)~%"
                          (second item) (getf (cddr item) :input))
                  (finish-output *error-output*))
                 (:cue
                  (when (plusp (getf runtime :volume))
                    (when (= (play-tone-sequence
                              (ten-seconds-cue-notes (second item))
                              (getf runtime :volume)) 1)
                      (setf (getf runtime :audio-owned-p) t))))
                 (:redraw (setf (getf runtime :dirty) t))
                 (otherwise (error "Unknown 10 Seconds effect ~S" item)))
               (record item))
             (dispatch (event now)
               (setf (getf runtime :now) now)
               (record (list event :now now))
               (multiple-value-bind (next effects)
                   (ten-seconds-reduce current event now)
                 (setf current next)
                 (mapc #'effect effects))))
      (unless (= (audio-active-p) 1)
        (setf (getf runtime :audio-owned-p) nil))
      (record '(:reap-sound))
      (setf tick-now (dashboard-runtime-read-clock runtime))
      (dispatch :tick tick-now)
      (when (getf runtime :dirty)
        (unless (render-ten-seconds current) (error "10 Seconds render failed"))
        (record '(:render))
        (unless (ten-seconds-runtime-present runtime)
          (error "10 Seconds presentation failed"))
        (record '(:present))
        (setf (getf runtime :dirty) nil))
      (let* ((poll (or (poll-native-input nil 8)
                       (error "10 Seconds native input poll failed")))
             (control-count (getf poll :control-count))
             (touch-count (getf poll :touch-count))
             (controller-a nil))
        (record '(:poll :wayland nil :timeout 8))
        (dotimes (index control-count)
          (let ((report (or (next-evdev-control)
                            (error "10 Seconds control queue ended early"))))
            (when (and (eq (getf report :kind) :gamepad)
                       (logtest #x004 (getf report :edges)))
              (setf controller-a t))))
        (let ((touches
                (loop repeat touch-count
                      collect (or (next-evdev-touch)
                                  (error "10 Seconds touch queue ended early")))))
          (record (list :controls control-count))
          (record (list :touches touch-count))
          (cond
            ((getf poll :touch-lost-p)
             (error "10 Seconds touchscreen disconnected"))
            ((getf poll :shutdown-p)
             (setf (getf runtime :running) nil)
             (record '(:shutdown)))
            ((some (lambda (report)
                     (and (fourth report)
                          (eq (ten-seconds-touch-event
                               (first report) (second report)) :back)))
                   touches)
             (dispatch :back tick-now))
            (controller-a
             (dispatch :controller-a (dashboard-runtime-read-clock runtime)))
            (t
             (dolist (report touches)
               (when (fourth report)
                 (dispatch :touch (dashboard-runtime-read-clock runtime))))))))
      (values current runtime (nreverse trace)))))

(defun ten-seconds-candidate-rehearse
    (state runtime &key (iteration-limit 1) stop-predicate)
  "Run an opt-in bounded 10 Seconds candidate and return iteration traces."
  (check-type iteration-limit (integer 0 *))
  (when stop-predicate (check-type stop-predicate function))
  (when (getf runtime :initialized-p)
    (error "10 Seconds runtime is already initialized"))
  (let ((current state) (iteration 0) (traces nil) (reason nil) (owned nil))
    (with-ten-seconds-cleanup ((when owned (ten-seconds-runtime-shutdown runtime)))
      (ten-seconds-runtime-initialize runtime)
      (setf owned t)
      (loop
        (cond
          ((not (getf runtime :running)) (setf reason :shutdown) (return))
          ((>= iteration iteration-limit) (setf reason :limit) (return))
          ((and stop-predicate
                (funcall stop-predicate current runtime iteration))
           (setf reason :operator-stop)
           (return)))
        (multiple-value-bind (next ignored-runtime trace)
            (ten-seconds-runtime-run-iteration current runtime)
          (declare (ignore ignored-runtime))
          (setf current next)
          (push trace traces)
          (incf iteration)))
      (values current runtime (nreverse traces) reason))))
