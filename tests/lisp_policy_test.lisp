(defpackage #:retrodeck.test
  (:use #:cl))
(in-package #:retrodeck.test)
(defmacro define-test-parameters (&body groups)
  `(progn
     ,@(loop for (initial-value . names) in groups append
             (loop for name in names
                   collect `(defparameter ,name ,initial-value)))))
(define-test-parameters
  (1 *play-status* *canvas-clear-status* *canvas-glyph-status*
     *canvas-fill-status* *projection-status* *projected-text-status*
     *control-file-write-status* *state-file-write-status*
     *chiptune-rewind-status* *chiptune-close-status*
     *chiptune-audio-open-status* *chiptune-audio-write-status*
     *chiptune-audio-close-status* *canvas-raster-status* *evdev-open-status* *fbdev-open-status*
     *fbdev-canvas-status* *fbdev-present-status* *wayland-open-status*
     *wayland-canvas-status* *wayland-present-status*)
  (0 *active-status* *active-count* *stop-count* *finish-count*
     *text-mask-result* *text-mask-clear-count* *raster-clear-count*
     *raster-cover-result* *raster-png-result* *evdev-controls-scan-count*
     *evdev-gamepads-scan-count* *evdev-controls-close-count* *evdev-open-count*
       *process-shutdown-status* *evdev-close-count*
     *evdev-dispatch-result* *fbdev-open-count* *fbdev-close-count*
     *fbdev-canvas-count* *wayland-open-count* *wayland-close-count*
     *wayland-canvas-count* *wayland-dispatch-result*
     *wayland-shutdown-status*)
  (nil *play-arguments* *record-interaction* *interaction-trace*
       *canvas-clear-color* *canvas-glyph-arguments* *canvas-glyph-calls*
       *canvas-fill-arguments* *projection-arguments*
       *projected-text-arguments* *projected-text-calls*
       *text-mask-arguments* *text-mask-calls* *regular-file-result*
       *regular-file-results* *regular-file-arguments* *regular-file-calls*
       *control-file-read-results* *control-file-read-paths*
       *control-file-write-arguments* *control-file-write-calls*
       *storage-write-trace* *state-file-read-results* *state-file-read-path*
       *state-file-read-paths* *state-file-write-arguments*
       *network-status-path* *canvas-fill-calls* *canvas-raster-arguments*
       *canvas-raster-calls* *raster-cover-arguments* *raster-cover-calls*
       *raster-png-arguments* *raster-png-calls*
       *evdev-controls-dispatch-timeout* *evdev-controls*
       *input-poll-arguments* *evdev-dispatch-timeout* *evdev-touch*
       *evdev-touch-queue* *fbdev-present-color* *fbdev-size*
       *wayland-open-display* *wayland-open-kind* *wayland-present-color*
       *wayland-dispatch-timeout* *wayland-touch* *wayland-touch-queue*
       *wayland-size* *helper-arguments* *terminal-arguments*
       *child-arguments* *chiptune-open-result* *chiptune-step-result*
       *chiptune-open-path* *chiptune-start-track-index*
       *chiptune-audio-open-volume* *chiptune-audio-write-pcm*
       *list-directory-path* *list-directory-result*
       *program-arguments-result* *canvas-waveform-arguments*)
      (0 *chiptune-rewind-count*)
  ("" *control-file-read-result*)
  ('(1 2 3 4) *canvas-hash-words* *monotonic-words*)
  ('(0) *state-file-read-result*)
  ('("" "" "" "STATUS UNAVAILABLE") *network-status-result*)
  ('(0 0) *evdev-controls-scan-result* *evdev-controls-dispatch-result*)
  ('(0 nil) *evdev-gamepads-scan-result*)
  ('(0 0 0 0 0 0 0) *input-poll-result*)
  ('(0 0 -1 nil) *helper-result*)
  ('(1 0 0 -1 nil 0) *terminal-result* *child-result*))
(defpackage #:retrodeck.native (:use))
(defmacro with-runtime-device-fixture
    ((&key (fbdev-size nil) (fbdev-open-status 1) (fbdev-canvas-status 1)
           (wayland-size nil) (wayland-open-status 1)
           (wayland-canvas-status 1) (evdev-open-status 1)
           (controls-scan-result ''(0 0)) (gamepads-scan-result ''(0 nil))
             (controls nil)
           (wayland-touch-queue nil))
     &body body)
  `(let ((*fbdev-size* ,fbdev-size)
         (*fbdev-open-status* ,fbdev-open-status)
         (*fbdev-open-count* 0)
         (*fbdev-close-count* 0)
         (*fbdev-canvas-status* ,fbdev-canvas-status)
         (*fbdev-canvas-count* 0)
         (*wayland-size* ,wayland-size)
         (*wayland-open-status* ,wayland-open-status)
         (*wayland-open-count* 0)
         (*wayland-open-display* nil)
          (*wayland-open-kind* nil)
         (*wayland-close-count* 0)
         (*wayland-canvas-status* ,wayland-canvas-status)
         (*wayland-canvas-count* 0)
         (*wayland-touch* nil)
         (*wayland-touch-queue* ,wayland-touch-queue)
         (*evdev-open-status* ,evdev-open-status)
          (*process-shutdown-status* 0)
         (*evdev-open-count* 0)
         (*evdev-close-count* 0)
         (*evdev-controls-scan-result* ,controls-scan-result)
          (*evdev-gamepads-scan-result* ,gamepads-scan-result)
          (*evdev-gamepads-scan-count* 0)
         (*evdev-controls-scan-count* 0)
         (*evdev-controls-close-count* 0)
         (*evdev-controls* ,controls)
         (*evdev-touch* nil)
         (*evdev-touch-queue* nil))
     ,@body))
(defmacro define-native-test-functions (&body definitions)
  `(progn
     ,@(loop for (name parameters . body) in definitions
             collect `(setf (symbol-function
                             (intern ,(string name) "RETRODECK.NATIVE"))
                            (lambda ,parameters ,@body)))))
(defmacro record-native-call (arguments last-call result &optional calls)
  `(progn
     (setf ,last-call ,arguments)
     ,@(when calls `((push ,arguments ,calls)))
     ,result))
(defun record-native-play (arguments)
  (setf *play-arguments* arguments)
  (when *record-interaction*
    (push :sound *interaction-trace*))
  *play-status*)
(define-native-test-functions
  (abi-version () 28)
  (program-arguments () *program-arguments-result*)
  (list-directory (path)
    (setf *list-directory-path* path)
    *list-directory-result*)
  (audio-active-p () (incf *active-count*) *active-status*)
  (chiptune-audio-open (volume)
    (setf *chiptune-audio-open-volume* volume)
    *chiptune-audio-open-status*)
  (chiptune-audio-write (pcm)
    (setf *chiptune-audio-write-pcm* pcm)
    *chiptune-audio-write-status*)
  (chiptune-audio-close () *chiptune-audio-close-status*)
  (chiptune-open (path)
    (setf *chiptune-open-path* path)
    *chiptune-open-result*)
  (chiptune-start-track (track)
    (setf *chiptune-start-track-index* track)
    *chiptune-open-result*)
  (chiptune-step () *chiptune-step-result*)
  (chiptune-rewind ()
    (incf *chiptune-rewind-count*)
    *chiptune-rewind-status*)
  (chiptune-close () *chiptune-close-status*)
  (process-shutdown-p () *process-shutdown-status*)
  (play-tones (&rest arguments) (record-native-play arguments))
  (play-tone-sequence (&rest arguments) (record-native-play arguments))
  (stop-audio () (incf *stop-count*)
    (if (eq *record-interaction* :cleanup-error) (error "stop failed") 0))
  (finish-audio () (incf *finish-count*) 0)
  (canvas-clear (color)
    (setf *canvas-clear-color* color)
    (when *record-interaction*
      (push :render *interaction-trace*))
    *canvas-clear-status*)
  (canvas-rgb565-hash-words () *canvas-hash-words*)
  (monotonic-nanoseconds-words () *monotonic-words*)
  (canvas-configure-projection (&rest arguments)
    (record-native-call arguments *projection-arguments* *projection-status*))
  (canvas-draw-projected-text (&rest arguments)
    (record-native-call arguments *projected-text-arguments*
                        *projected-text-status* *projected-text-calls*))
  (canvas-draw-glyph (&rest arguments)
    (record-native-call arguments *canvas-glyph-arguments*
                        *canvas-glyph-status* *canvas-glyph-calls*))
  (canvas-draw-waveform (&rest arguments)
    (setf *canvas-waveform-arguments* arguments)
    *canvas-fill-status*)
  (canvas-fill-rect (&rest arguments)
    (record-native-call arguments *canvas-fill-arguments*
                        *canvas-fill-status* *canvas-fill-calls*))
  (canvas-draw-raster (&rest arguments)
    (record-native-call arguments *canvas-raster-arguments*
                        *canvas-raster-status* *canvas-raster-calls*))
  (raster-clear () (incf *raster-clear-count*) 1)
  (raster-load-cover (&rest arguments)
    (record-native-call arguments *raster-cover-arguments*
                        *raster-cover-result* *raster-cover-calls*))
  (raster-load-png (&rest arguments)
    (record-native-call arguments *raster-png-arguments*
                        *raster-png-result* *raster-png-calls*))
  (read-control-file (path)
    (push path *control-file-read-paths*)
    (let ((entry (assoc path *control-file-read-results* :test #'string=)))
      (if entry (cdr entry) *control-file-read-result*)))
  (read-regular-file (&rest arguments)
    (setf *regular-file-arguments* arguments)
    (push arguments *regular-file-calls*)
    (let ((entry (assoc (first arguments) *regular-file-results*
                        :test #'string=)))
      (if entry (cdr entry) *regular-file-result*)))
  (read-state-file (path)
    (setf *state-file-read-path* path)
    (push path *state-file-read-paths*)
    (let ((entry (assoc path *state-file-read-results* :test #'string=)))
      (if entry (cdr entry) *state-file-read-result*)))
  (write-control-file (&rest arguments)
    (setf *control-file-write-arguments* arguments)
    (push arguments *control-file-write-calls*)
    (push (cons :control arguments) *storage-write-trace*)
    *control-file-write-status*)
  (write-state-file (&rest arguments)
    (setf *state-file-write-arguments* arguments)
    (push (cons :state arguments) *storage-write-trace*)
    *state-file-write-status*)
  (network-status (path)
    (record-native-call path *network-status-path* *network-status-result*))
  (run-helper (&rest arguments)
    (record-native-call arguments *helper-arguments* *helper-result*))
  (run-child (&rest arguments)
    (record-native-call arguments *child-arguments* *child-result*))
  (run-terminal (&rest arguments)
    (record-native-call arguments *terminal-arguments* *terminal-result*))
  (text-mask-clear () (incf *text-mask-clear-count*) 1)
  (text-mask-load (&rest arguments)
    (record-native-call arguments *text-mask-arguments*
                        *text-mask-result* *text-mask-calls*))
  (input-poll (wayland timeout-ms)
    (record-native-call (list wayland timeout-ms)
                        *input-poll-arguments* *input-poll-result*))
  (evdev-controls-scan ()
    (incf *evdev-controls-scan-count*)
    *evdev-controls-scan-result*)
  (evdev-gamepads-scan ()
    (incf *evdev-gamepads-scan-count*)
    *evdev-gamepads-scan-result*)
  (evdev-controls-close () (incf *evdev-controls-close-count*) 0)
  (evdev-controls-dispatch (timeout-ms)
    (record-native-call timeout-ms *evdev-controls-dispatch-timeout*
                        *evdev-controls-dispatch-result*))
  (evdev-next-control () (pop *evdev-controls*))
  (evdev-touch-open () (incf *evdev-open-count*) *evdev-open-status*)
  (evdev-touch-close ()
    (incf *evdev-close-count*)
    (when *record-interaction*
      (push :touch-close *interaction-trace*))
    0)
  (evdev-touch-dispatch (timeout-ms)
    (record-native-call timeout-ms *evdev-dispatch-timeout*
                        *evdev-dispatch-result*))
  (evdev-next-touch () (or (pop *evdev-touch-queue*) *evdev-touch*))
  (fbdev-open () (incf *fbdev-open-count*) *fbdev-open-status*)
  (fbdev-close () (incf *fbdev-close-count*) 0)
  (fbdev-present-canvas ()
    (incf *fbdev-canvas-count*)
    (when *record-interaction*
      (push :present *interaction-trace*))
    *fbdev-canvas-status*)
  (fbdev-present-solid (color)
    (record-native-call color *fbdev-present-color* *fbdev-present-status*))
  (fbdev-size () *fbdev-size*)
  (wayland-open-widget ()
    (incf *wayland-open-count*)
    (setf *wayland-open-display* :environment *wayland-open-kind* :widget)
    *wayland-open-status*)
  (wayland-open-widget-at (display)
    (incf *wayland-open-count*)
    (setf *wayland-open-display* display *wayland-open-kind* :widget)
    *wayland-open-status*)
  (wayland-open-gameplay-at (display)
    (incf *wayland-open-count*)
    (setf *wayland-open-display* display *wayland-open-kind* :gameplay)
    *wayland-open-status*)
  (wayland-close () (incf *wayland-close-count*) 0)
  (wayland-present-canvas ()
    (incf *wayland-canvas-count*)
    (when *record-interaction*
      (push :present *interaction-trace*))
    *wayland-canvas-status*)
  (wayland-present-solid (color)
    (record-native-call color *wayland-present-color* *wayland-present-status*))
  (wayland-dispatch (timeout-ms)
    (record-native-call timeout-ms *wayland-dispatch-timeout*
                        *wayland-dispatch-result*))
  (wayland-next-touch () (or (pop *wayland-touch-queue*) *wayland-touch*))
  (wayland-size () *wayland-size*)
  (wayland-shutdown-p () *wayland-shutdown-status*))
(load (truename (merge-pathnames "../lisp/startup.lisp" *load-truename*))
      :verbose nil :print nil)
(defmacro signals-p (condition &body body)
  `(handler-case (progn ,@body nil)
     (,condition () t)))
(defmacro assert-signals (condition &body body)
  `(assert (signals-p ,condition ,@body)))
(defun runtime-test-games (&optional include-beta-p)
  (let ((games
          '((:id "alpha" :title "ALPHA" :system :nes
             :color #x5f87ff :rom "/tmp/alpha.nes")
            (:id "beta" :title "BETA" :system :nes
             :color #xafd75f :rom "/tmp/beta.nes"))))
    (copy-tree (if include-beta-p games (subseq games 0 1)))))
(defun test-file-string (path)
  (with-open-file (input path)
    (let ((contents (make-string (file-length input))))
      (read-sequence contents input)
      contents)))
(defun test-menu-path (name &optional namestring-p)
  (let ((path (truename (merge-pathnames name
                                          (merge-pathnames "../deploy/menu/"
                                                           *load-truename*)))))
    (if namestring-p (namestring path) path)))
(defun test-line (text)
  (format nil "~A~%" text))
(defun test-state-result (text)
  (list 1 (test-line text)))
(defun decode-native-unsigned-64 (text)
  (assert (= (length text) 16))
  (parse-integer text :radix 16))
(defun assert-unary-table (test parser fixtures &rest fixed-arguments)
  (dolist (fixture fixtures)
    (assert (funcall test (apply parser (first fixture) fixed-arguments)
                     (second fixture)))))
(defun assert-binary-table (test parser fixtures)
  (dolist (fixture fixtures)
    (assert (funcall test (funcall parser (first fixture) (second fixture))
                     (third fixture)))))
(defmacro assert-signaled-table (condition function values)
  `(dolist (value ,values)
     (assert-signals ,condition (funcall ,function value))))
(defmacro assert-recorded-calls (variable form expected)
  `(progn
     (setf ,variable nil)
     (assert ,form)
     (assert (equal (nreverse ,variable) ,expected))))
(defmacro assert-native-call (form expected variable arguments)
  `(progn (assert (equal ,form ,expected))
          (assert (equal ,variable ,arguments))))
(defmacro assert-native-writer (function status-variable arguments-variable path contents)
  `(progn
     (setf ,status-variable 1 ,arguments-variable nil)
     (assert (,function ,path ,contents))
     (assert (equal ,arguments-variable (list ,path ,contents)))
     (setf ,status-variable 0)
     (assert (not (,function ,path "0")))
     (assert-signals type-error (,function "/tmp/x" 4))))
(defmacro assert-values (form &rest expected)
  `(assert (equal (multiple-value-list ,form) (list ,@expected))))
(defmacro assert-touch-release
    (transition state layout press release &body assertions)
  `(multiple-value-bind (pressed press-effect)
       (,transition ,state ,layout ,press)
     (assert (null press-effect))
     (multiple-value-bind (released release-effect)
         (,transition pressed ,layout ,release)
       ,@assertions)))
(defun assert-rect-target-boundaries (finder layout target)
  (destructuring-bind (x y width height) (getf layout target)
    (assert (eq (funcall finder layout x y) target))
    (assert (eq (funcall finder layout (+ x width -1) (+ y height -1)) target))
    (assert (not (eq (funcall finder layout (+ x width) y) target)))
    (assert (not (eq (funcall finder layout x (+ y height)) target)))))
(defun assert-settings-completion
    (state plan succeeded-p expected-effect fields
     &key (tone-succeeded-p t tone-supplied-p))
  (multiple-value-bind (result effect)
      (if tone-supplied-p
          (retrodeck:settings-complete-action
           state plan succeeded-p :tone-succeeded-p tone-succeeded-p)
          (retrodeck:settings-complete-action state plan succeeded-p))
    (assert (if expected-effect (equal effect expected-effect) (null effect)))
    (dolist (field fields)
      (assert (apply (first field) (getf result (second field)) (cddr field))))
    result))
(defun runtime-effect (runtime effect state)
  (retrodeck::dashboard-runtime-handle-effect runtime effect state))
(defun assert-runtime-effect (runtime effect state expected)
  (assert (equal (runtime-effect runtime effect state) expected)))
(defun assert-runtime-write (runtime effect state expected path text)
  (assert-runtime-effect runtime effect state expected)
  (assert (equal *state-file-write-arguments* (list path text))))
(defmacro with-initialized-dashboard-runtime ((state runtime now) &body body)
  `(multiple-value-bind (initialized returned-runtime)
       (retrodeck:dashboard-runtime-initialize ,state ,runtime ,now)
     (declare (ignorable initialized))
     (assert (eq returned-runtime ,runtime))
     ,@body))
(defun dashboard-runtime-observation (runtime name)
  (ecase name
    (:active-count *active-count*) (:audio-owned (getf runtime :audio-owned-p))
    (:brightness-maximum (getf runtime :brightness-maximum))
    (:controls *evdev-controls*) (:controls-close *evdev-controls-close-count*)
    (:controls-owned (getf runtime :controls-owned-p))
    (:controls-scan *evdev-controls-scan-count*) (:dirty (getf runtime :dirty))
    (:evdev-close *evdev-close-count*) (:evdev-open *evdev-open-count*)
    (:fbdev-canvas *fbdev-canvas-count*) (:fbdev-close *fbdev-close-count*)
    (:fbdev-open *fbdev-open-count*) (:finish-count *finish-count*)
    (:gamepads-scan *evdev-gamepads-scan-count*)
    (:initialized (getf runtime :initialized-p)) (:input-poll *input-poll-arguments*)
    (:layout (getf runtime :layout))
    (:menu-sound-until retrodeck::*menu-sound-input-until-ms*)
    (:presentation-owned (getf runtime :presentation-owned-p))
    (:running (retrodeck:dashboard-runtime-running-p runtime))
    (:sound-active (getf runtime :sound-active-p)) (:stop-count *stop-count*)
    (:touch-owned (getf runtime :touch-owned-p)) (:touches *evdev-touch-queue*)
    (:wayland (getf runtime :wayland)) (:wayland-canvas *wayland-canvas-count*)
    (:wayland-close *wayland-close-count*) (:wayland-display *wayland-open-display*)
    (:wayland-kind *wayland-open-kind*) (:wayland-open *wayland-open-count*)))
(defun assert-dashboard-runtime-observations (runtime &rest expected)
  (loop for (name value) on expected by #'cddr
        do (assert (equal (dashboard-runtime-observation runtime name) value))))
(defmacro assert-runtime-observations (&rest expected)
  `(assert-dashboard-runtime-observations runtime ,@expected))
(defun assert-plist-values (plist expected)
  (loop for (name value) on expected by #'cddr
        do (assert (equal (getf plist name) value))))
(defun assert-ten-seconds-reduction (state input now effects &rest expected)
  (multiple-value-bind (next actual-effects)
      (retrodeck:ten-seconds-reduce state input now)
    (assert (equal actual-effects effects))
    (loop for (name value) on expected by #'cddr
          do (if (eq name :identity)
                 (assert (eq next value))
                 (assert (equal (getf next name) value))))
    next))
(defmacro with-dashboard-runtime-fixture
    ((state runtime games state-now initialize-now state-options runtime-options)
     device-options &body body)
  `(let ((,state (retrodeck:dashboard-loop-initial-state
                  ,games :now ,state-now ,@state-options))
         (,runtime (retrodeck:make-dashboard-runtime ,@runtime-options)))
     (with-runtime-device-fixture ,device-options
       (with-initialized-dashboard-runtime (,state ,runtime ,initialize-now)
         ,@body))))
(defmacro with-runtime-begin ((state trace) current runtime input &body body)
  `(multiple-value-bind (,state ,trace)
       (retrodeck:dashboard-runtime-begin-iteration ,current ,runtime ,input) ,@body))
(defmacro with-runtime-dispatch ((state trace) current runtime input &body body)
  `(multiple-value-bind (,state ,trace)
       (retrodeck:dashboard-runtime-dispatch-input ,current ,runtime ,input) ,@body))
(defmacro assert-dashboard-runtime-initialization-failure
    (now runtime-options device-options diagnostic &rest observations)
  `(let ((state (retrodeck:dashboard-loop-initial-state nil :now ,now))
         (runtime (retrodeck:make-dashboard-runtime ,@runtime-options))
         (diagnostics (make-string-output-stream)))
     (with-runtime-device-fixture ,device-options
       (let ((*error-output* diagnostics))
         (assert-signals error
           (retrodeck:dashboard-runtime-initialize state runtime ,now)))
       (when ,diagnostic
         (assert (search ,diagnostic (get-output-stream-string diagnostics))))
       (assert-dashboard-runtime-observations runtime ,@observations))))
(defun check-dashboard-startup-storage-failure
    (stage maximum current brightness keymap
     &key (control-write-status 1) (state-write-status 1))
  (let* ((keymap-p (eq stage :keymap))
         (state (apply #'retrodeck:dashboard-loop-initial-state nil :now 1
                       (unless keymap-p '(:brightness 40))))
         (runtime (retrodeck:make-dashboard-runtime
                   :volume-state "/tmp/volume.state" :default-volume 42
                   :brightness-device "/tmp/brightness"
                   :brightness-maximum-path "/tmp/max_brightness"
                   :brightness-state "/tmp/brightness.state"
                   :keymap-state "/tmp/keymap.state"))
         (*control-file-read-result* nil) (*control-file-read-paths* nil)
         (*control-file-read-results*
           (list (cons "/tmp/max_brightness" (and maximum (test-line maximum)))
                 (cons "/tmp/brightness" (and current (test-line current)))))
         (*control-file-write-status* control-write-status)
         (*control-file-write-calls* nil)
         (*state-file-read-result* (test-state-result "42"))
         (*state-file-read-results*
           (list (cons "/tmp/brightness.state" (test-state-result brightness))
                 (cons "/tmp/keymap.state" (test-state-result keymap))))
         (*state-file-read-paths* nil) (*state-file-write-status* state-write-status)
         (*state-file-write-arguments* nil) (*storage-write-trace* nil)
         (*fbdev-open-count* 0) (*evdev-open-count* 0)
         (*evdev-controls-scan-count* 0)
         (control-write (list :control "/tmp/brightness" (test-line "12")))
         (state-write (list :state "/tmp/brightness.state" (test-line "60")))
         (control-reads (if (member stage '(:maximum-zero :maximum-missing))
                            '("/tmp/max_brightness")
                            '("/tmp/max_brightness" "/tmp/brightness")))
         (state-reads (case stage
                        (:keymap '("/tmp/volume.state" "/tmp/brightness.state"
                                   "/tmp/keymap.state"))
                        ((:brightness-state :control-write :state-write)
                         '("/tmp/volume.state" "/tmp/brightness.state"))
                        (otherwise '("/tmp/volume.state"))))
         (write-trace (case stage
                        (:control-write (list control-write))
                        (:state-write (list control-write state-write)))))
    (assert-signals error (retrodeck:dashboard-runtime-initialize state runtime 1))
    (assert (equal (reverse *control-file-read-paths*) control-reads))
    (assert (equal (reverse *state-file-read-paths*) state-reads))
    (if keymap-p
        (progn
          (assert (equal (reverse *control-file-write-calls*)
                         (list (rest control-write))))
          (assert (equal *state-file-write-arguments* (rest state-write))))
        (progn
          (assert (equal (reverse *storage-write-trace*) write-trace))
          (assert (= (getf (getf state :settings) :brightness) 40))))
    (assert-runtime-observations :brightness-maximum nil :fbdev-open 0 :evdev-open 0
     :controls-scan 0 :running nil :initialized nil)))
(defmacro check-dashboard-state-load
    (kind result expected &key default (write nil write-p) (write-status 1))
  (let* ((volume-p (eq kind :volume))
         (path (if volume-p "/tmp/volume.state" "/tmp/keymap.state"))
         (loader (if volume-p `(retrodeck:load-dashboard-volume-state ,path ,default)
                     `(retrodeck:load-dashboard-keymap-state ,path))))
    `(let ((*state-file-read-result* ,result) (*state-file-write-status* ,write-status)
           (*state-file-write-arguments* nil))
       ,(if (eq expected :error) `(assert-signals error ,loader)
            `(assert (,(if volume-p '= 'string=) ,loader ,expected)))
       ,@(when write-p
           `((assert (equal *state-file-write-arguments*
                            (and ,write (list ,path (test-line ,write))))))))))
(defmacro check-dashboard-brightness-set
    (control-status state-status percent expected expected-kinds
     &key write-values check-state-write-p)
  `(let ((*control-file-write-status* ,control-status)
         (*state-file-write-status* ,state-status) (*storage-write-trace* nil)
         (*control-file-write-arguments* nil) (*state-file-write-arguments* nil))
     ,(if (eq expected :error)
          `(assert-signals error (retrodeck:set-dashboard-brightness-percent
                                  "/tmp/brightness" "/tmp/brightness.state" 20 ,percent))
          `(assert (= (retrodeck:set-dashboard-brightness-percent
                       "/tmp/brightness" "/tmp/brightness.state" 20 ,percent) ,expected)))
     (let ((trace (reverse *storage-write-trace*)))
       (assert (equal (mapcar #'first trace) ,expected-kinds))
       ,@(when write-values
           `((assert (equal trace (mapcar #'list '(:control :state)
                                           '("/tmp/brightness" "/tmp/brightness.state")
                                           (mapcar #'test-line ,write-values)))))))
     ,@(when check-state-write-p '((assert (null *state-file-write-arguments*))))))
(defmacro with-ten-seconds-runtime-fixture
    ((runtime runtime-options device-options &rest bindings) &body body)
  `(let* (,@bindings
          (,runtime (retrodeck:make-ten-seconds-runtime ,@runtime-options)))
     (with-runtime-device-fixture ,device-options
       (let ((*error-output* (make-broadcast-stream)))
         ,@body))))
(defun exercise-ten-seconds-runtime
    (controls touches times &key wayland (active 0) (play 1))
  (let* ((runtime (retrodeck:make-ten-seconds-runtime
                   :presentation (and wayland "layer-shell")
                   :wayland-display (and wayland "wayland-test")
                   :clock (lambda () (or (pop times) 999))))
         (*active-status* active) (*active-count* 0)
         (*play-status* play) (*play-arguments* nil) (*stop-count* 0)
         (*input-poll-result* (list 1 (length controls) (length touches) 0 1 0 0))
         (*input-poll-arguments* nil) (*canvas-clear-status* 1)
         (*canvas-glyph-status* 1) (*canvas-fill-status* 1))
    (with-runtime-device-fixture (:controls controls)
      (let ((*error-output* (make-broadcast-stream))
            (*evdev-touch-queue* touches)
            (retrodeck::*menu-sound-input-until-ms* most-positive-fixnum))
        (retrodeck:ten-seconds-runtime-initialize runtime)
        (assert (zerop *wayland-open-count*))
        (multiple-value-bind (state ignored-runtime trace)
            (retrodeck:ten-seconds-runtime-run-iteration
             (retrodeck:ten-seconds-initial-state) runtime)
          (declare (ignore ignored-runtime))
          (let ((owned (getf runtime :audio-owned-p)))
            (retrodeck:ten-seconds-runtime-shutdown runtime)
            (assert-runtime-observations :active-count 1 :gamepads-scan 1 :controls-scan 0
             :controls nil :touches nil :input-poll '(0 8) :evdev-open 1
             :fbdev-close (if wayland 0 1) :wayland-close (if wayland 1 0)
             :wayland-kind (and wayland :gameplay)
             :menu-sound-until most-positive-fixnum)
            (values state trace
                    (list :owned owned :play *play-arguments*
                          :poll *input-poll-arguments* :stops *stop-count*))))))))
(assert-unary-table #'equal #'retrodeck:menu-sound-notes
                    '((:volume ((660 60) (880 60))) (:previous ((523 35)))
                      (:next ((659 35))) (:confirm ((659 25) (880 30)))
                      (:unknown ((659 25) (440 30)))))
(assert-unary-table #'= #'retrodeck:menu-sound-duration-ms
                    '((:volume 120) (:confirm 55)))
(assert (= retrodeck:*menu-sound-input-tail-ms* 60))
(let ((before (retrodeck::monotonic-ms)))
  (setf *play-status* 1)
  (assert-values (retrodeck:play-menu-sound :confirm 42) t t)
  (let ((after (retrodeck::monotonic-ms)))
    (assert (<= (+ before 115) retrodeck::*menu-sound-input-until-ms*
                (+ after 115)))))
(assert (equal *play-arguments* '(659 25 880 30 42)))
(dolist (fixture
         '((:previous 17 1 0 (t t) (523 35 0 0 17) nil)
           (:next 42 2 77 (t nil) (659 35 0 0 42) t)
           (:next 42 0 77 (nil nil) (659 35 0 0 42) t)
           (:next 0 0 77 (t nil) nil t)))
  (destructuring-bind (sound volume status input-until values arguments
                       check-input-until-p) fixture
    (setf *play-status* status *play-arguments* nil
          retrodeck::*menu-sound-input-until-ms* input-until)
    (assert (equal (multiple-value-list (retrodeck:play-menu-sound sound volume))
                   values))
    (assert (equal *play-arguments* arguments))
    (when check-input-until-p
      (assert (= retrodeck::*menu-sound-input-until-ms* input-until)))))
(let ((notes '((784 35) (1047 40) (1319 55))))
  (setf *play-status* 1)
  (assert (= (retrodeck.native:play-tone-sequence notes 42) 1))
  (assert (equal *play-arguments* (list notes 42))))
(setf *active-status* 1 retrodeck::*menu-sound-input-until-ms* 0)
(assert-binary-table #'eq #'retrodeck:menu-sound-blocks-input-p
                     '((:controller 100 t) (:touch 100 nil)
                       (:keyboard 100 nil)))
(setf *active-status* 0 retrodeck::*menu-sound-input-until-ms* 100)
(assert-binary-table #'eq #'retrodeck:menu-sound-blocks-input-p
                     '((:controller 99 t) (:controller 100 nil)))
(dolist (function (list #'retrodeck:stop-menu-sound
                        #'retrodeck:finish-menu-sound))
  (setf retrodeck::*menu-sound-input-until-ms* 100)
  (assert (funcall function))
  (assert (zerop retrodeck::*menu-sound-input-until-ms*)))
(assert (= *stop-count* *finish-count* 1))

(assert (retrodeck:clear-canvas #x121212))
(assert (= *canvas-clear-color* #x121212))
(assert (= (retrodeck:canvas-rgb565-hash) #x0001000200030004))
(assert (= (retrodeck:monotonic-nanoseconds) #x0001000200030004))
(assert-native-call (retrodeck:draw-canvas-glyph -4 8 65 2 #xfe6c27) t
                    *canvas-glyph-arguments* '(-4 8 65 2 #xfe6c27))
(assert (retrodeck:draw-canvas-glyph 0 0 0 1 0))
(assert (retrodeck:draw-canvas-glyph 0 0 255 1 #xffffff))
(setf *canvas-glyph-status* 0)
(assert (not (retrodeck:draw-canvas-glyph 0 0 65 1 0)))
(setf *canvas-glyph-status* 1 *canvas-glyph-arguments* nil)
(dolist (arguments '((#x80000000 0 65 1 0) (0 0 256 1 0)
                     (0 0 65 0 0) (0 0 65 #x100000000 0)))
  (assert-signals type-error (apply #'retrodeck:draw-canvas-glyph arguments)))
(assert (null *canvas-glyph-arguments*))
(assert-native-call (retrodeck:fill-canvas-rect -4 8 12 16 #xfe6c27) t
                    *canvas-fill-arguments* '(-4 8 12 16 #xfe6c27))
(setf *canvas-fill-arguments* nil)
(dolist (arguments '((#x80000000 0 1 1 0) (0 0 #x100000000 1 0)))
  (assert-signals type-error (apply #'retrodeck:fill-canvas-rect arguments)))
(assert (null *canvas-fill-arguments*))

(setf *regular-file-result* "project\trole\tlicense\n")
(assert-native-call (retrodeck:read-bounded-regular-file "/tmp/credits.tsv" 1 32768)
                    *regular-file-result* *regular-file-arguments*
                    '("/tmp/credits.tsv" 1 32768))
(assert-signals type-error (retrodeck:read-bounded-regular-file "/tmp/x" -1 2))
(assert-signals type-error (retrodeck:read-bounded-regular-file "/tmp/x" 1 4194305))

(setf *control-file-read-result* (test-line "12")
      *control-file-read-paths* nil)
(assert-native-call (retrodeck:read-native-control-file "/tmp/brightness")
                    (test-line "12") *control-file-read-paths*
                    '("/tmp/brightness"))
(setf *control-file-read-result* nil)
(assert-signals error (retrodeck:read-native-control-file "/tmp/brightness"))
(assert-signals type-error (retrodeck:read-native-control-file 4))
(setf *control-file-write-calls* nil)
(assert-native-writer retrodeck:write-native-control-file
                      *control-file-write-status* *control-file-write-arguments*
                      "/tmp/brightness" (test-line "14"))

(setf *state-file-read-result* '(0))
(assert-values (retrodeck:read-native-state-file "/tmp/volume.state") nil nil)
(assert (string= *state-file-read-path* "/tmp/volume.state"))
(let ((contents (test-line "42")))
  (setf *state-file-read-result* (list 1 contents))
  (assert-values (retrodeck:read-native-state-file "/tmp/volume.state")
                 contents t))
(dolist (invalid '(nil (0 nil) (1) (1 42) (2)))
  (setf *state-file-read-result* invalid)
  (assert-signals error (retrodeck:read-native-state-file "/tmp/volume.state")))
(assert-signals type-error (retrodeck:read-native-state-file 4))
(assert-native-writer retrodeck:write-native-state-file
                      *state-file-write-status* *state-file-write-arguments*
                      "/tmp/volume.state" (test-line "37"))
(assert-unary-table #'= #'retrodeck:parse-dashboard-volume-state
                    (list (list (test-line "0") 0)
                          (list (test-line "5") 5)
                          (list (test-line "42") 42)
                          (list (test-line "100") 100)))
(assert-signaled-table error #'retrodeck:parse-dashboard-volume-state
                       (list "" (format nil "~%") (test-line "00")
                             (test-line "042") (test-line "101") "42"
                             (format nil "42~%0") (format nil "42~%~%")
                             (test-line "-1") (test-line "on")))
(assert-signals type-error (retrodeck:parse-dashboard-volume-state 42))
(assert (= (retrodeck:parse-dashboard-inherited-volume nil) 42))
(assert-unary-table #'= #'retrodeck:parse-dashboard-inherited-volume
                    '(("0" 0) ("00" 0) ("042" 42) ("100" 100)))
(assert-signaled-table error #'retrodeck:parse-dashboard-inherited-volume
                       '("" "loud" "101" "-1" "1x"))
(let ((retrodeck:*dashboard-volume-default* 0))
  (assert (zerop (retrodeck:parse-dashboard-inherited-volume nil))))
(assert (= (retrodeck:dashboard-inherited-volume) 42))

(check-dashboard-state-load :volume '(0) 42 :default 42 :write "42")
(check-dashboard-state-load :volume (test-state-result "37") 37
                            :default 42 :write nil)
(check-dashboard-state-load :volume (test-state-result "on") 37
                            :default 37 :write "37")
(check-dashboard-state-load :volume (test-state-result "off") 0
                            :default 37 :write "0")
(check-dashboard-state-load :volume (test-state-result "042") :error
                            :default 42 :write nil)
(check-dashboard-state-load :volume '(0) :error :default 42 :write-status 0)
(setf *state-file-write-status* 1
      *state-file-write-arguments* nil)
(assert (retrodeck:save-dashboard-volume-state "/tmp/volume.state" 63))
(assert (equal *state-file-write-arguments*
               (list "/tmp/volume.state" (test-line "63"))))
(assert-signals type-error
                (retrodeck:save-dashboard-volume-state "/tmp/volume.state" 101))
(assert-unary-table #'= #'retrodeck::parse-dashboard-control-integer
                    (list (list (test-line "12") 12)
                          (list (format nil "~C~C~C12~C~C~C"
                                             #\Space #\Tab (code-char 11)
                                             (code-char 12) #\Return #\Newline)
                                12)
                          '("00020" 20) '("4294967295" 4294967295))
                    "brightness")
(assert-signaled-table
 error (lambda (value)
         (retrodeck::parse-dashboard-control-integer value "brightness"))
 (list "" (format nil " ~C~C~%" #\Tab #\Return)
       "-1" "12x" "4294967296"))
(assert-unary-table #'= #'retrodeck:parse-dashboard-brightness-state
                    (list (list (test-line "10") 10)
                          (list (test-line "60") 60)
                          (list (test-line "100") 100)))
(assert-signaled-table error #'retrodeck:parse-dashboard-brightness-state
                       (list "" (test-line "0") (test-line "5")
                             (test-line "05") (test-line "55")
                             (test-line "110") "60"
                             (format nil "60~%~%") (format nil " 60~%")))
(assert (= (retrodeck::dashboard-brightness-raw-value 10 20) 2))
(assert (= (retrodeck::dashboard-brightness-raw-value 60 20) 12))
(assert (= (retrodeck::dashboard-brightness-raw-value 100 20) 20))
(assert (= (retrodeck::dashboard-brightness-raw-value 10 1) 1))
(assert (zerop (retrodeck::dashboard-brightness-raw-value 60 0)))
(assert-binary-table #'= #'retrodeck::dashboard-observed-brightness-percent
                     '((0 20 10) (1 20 10) (12 20 60)
                       (13 20 70) (19 20 100) (20 20 100)))
(assert-signals error
                (retrodeck::dashboard-observed-brightness-percent 21 20))
(check-dashboard-brightness-set
 1 1 70 70 '(:control :state) :write-values '("14" "70"))
(check-dashboard-brightness-set
 0 1 70 :error '(:control) :check-state-write-p t)
(check-dashboard-brightness-set 1 0 70 :error '(:control :state))
(check-dashboard-brightness-set 1 1 65 :error nil)
(setf *control-file-read-results*
      (list (cons "/tmp/max_brightness" (test-line "20"))
            (cons "/tmp/brightness" (test-line "12")))
      *control-file-read-paths* nil
      *state-file-read-results* (list (cons "/tmp/brightness.state" '(0)))
      *state-file-read-paths* nil
      *control-file-write-status* 1
      *state-file-write-status* 1
      *storage-write-trace* nil)
(assert-values (retrodeck:load-dashboard-brightness
                "/tmp/brightness" "/tmp/max_brightness" "/tmp/brightness.state")
               60 20)
(assert (equal (reverse *control-file-read-paths*)
               '("/tmp/max_brightness" "/tmp/brightness")))
(assert (equal (reverse *state-file-read-paths*)
               '("/tmp/brightness.state")))
(assert (equal (reverse *storage-write-trace*)
               (list (list :control "/tmp/brightness" (test-line "12"))
                     (list :state "/tmp/brightness.state"
                           (test-line "60")))))
(setf *state-file-read-results*
      (list (cons "/tmp/brightness.state" (test-state-result "70")))
      *storage-write-trace* nil)
(assert-values (retrodeck:load-dashboard-brightness
                "/tmp/brightness" "/tmp/max_brightness" "/tmp/brightness.state")
               70 20)
(assert (equal (reverse *storage-write-trace*)
               (list (list :control "/tmp/brightness" (test-line "14"))
                     (list :state "/tmp/brightness.state"
                           (test-line "70")))))
(setf *state-file-read-results*
      (list (cons "/tmp/brightness.state" (test-state-result "05")))
      *storage-write-trace* nil)
(assert-signals error
                (retrodeck:load-dashboard-brightness
                 "/tmp/brightness" "/tmp/max_brightness"
                 "/tmp/brightness.state"))
(assert (null *storage-write-trace*))
(setf *control-file-read-results*
      (list (cons "/tmp/max_brightness" (test-line "0"))
            (cons "/tmp/brightness" (test-line "12")))
      *control-file-read-paths* nil
      *state-file-read-paths* nil)
(assert-signals error
                (retrodeck:load-dashboard-brightness
                 "/tmp/brightness" "/tmp/max_brightness"
                 "/tmp/brightness.state"))
(assert (equal (reverse *control-file-read-paths*) '("/tmp/max_brightness")))
(assert (null *state-file-read-paths*))

(assert-unary-table #'string= #'retrodeck:parse-dashboard-keymap-state
                    (list (list (test-line "us") "us")
                          (list (test-line "cz") "cz")))
(assert-signaled-table error #'retrodeck:parse-dashboard-keymap-state
                       (list "" "us" (test-line "US") (test-line "de")
                             (format nil "us~%~%") (format nil "us ~%")
                             (format nil "us~C~%" #\Return)))
(check-dashboard-state-load :keymap '(0) "us" :write "us")
(check-dashboard-state-load :keymap (test-state-result "cz") "cz" :write nil)
(check-dashboard-state-load :keymap (test-state-result "de") :error)
(check-dashboard-state-load :keymap '(0) :error :write-status 0)
(setf *state-file-write-status* 1
      *state-file-write-arguments* nil)
(assert (retrodeck:save-dashboard-keymap-state "/tmp/keymap.state" "cz"))
(assert (equal *state-file-write-arguments*
               (list "/tmp/keymap.state" (test-line "cz"))))
(assert-signals error
                (retrodeck:save-dashboard-keymap-state "/tmp/keymap.state" "de"))
(setf *control-file-read-result* ""
      *control-file-read-results*
      (list (cons "/sys/class/backlight/display-bl/max_brightness"
                  (test-line "20"))
            (cons "/sys/class/backlight/display-bl/brightness"
                  (test-line "12")))
      *control-file-read-paths* nil
      *control-file-write-status* 1
      *state-file-read-result* (test-state-result "42")
      *state-file-read-results*
      (list (cons "/mnt/data/nes-deck/state/menu-brightness.state"
                  (test-state-result "60"))
            (cons "/mnt/data/nes-deck/state/terminal-keymap.state"
                  (test-state-result "us")))
      *state-file-read-paths* nil
      *state-file-write-status* 1
      *storage-write-trace* nil)

(setf *network-status-result*
      '("NET1" "10.0.1.11" "10.0.0.15" "CONNECTED"))
(assert (equal (retrodeck:read-native-network-status "/tmp/wifi-status")
               '(:ssid "NET1" :wlan-ipv4 "10.0.1.11"
                 :wireguard-ipv4 "10.0.0.15" :selector "CONNECTED")))
(assert (string= *network-status-path* "/tmp/wifi-status"))
(dolist (invalid '(nil ("" "" "") ("" "" "" 4)))
  (setf *network-status-result* invalid)
  (assert-signals error
                  (retrodeck:read-native-network-status "/tmp/wifi-status")))
(assert-signals type-error (retrodeck:read-native-network-status 4))
(setf *network-status-result* '("" "" "" "STATUS UNAVAILABLE"))

(setf *text-mask-result* 17)
(assert (= (retrodeck:load-text-mask "HH" 4) 17))
(assert (equal *text-mask-arguments* '("HH" 4)))
(assert (retrodeck:configure-text-projection
         2000 1 20 8044 420 4000 56 72 104 210 480 #xffffaf))
(assert (= (decode-native-unsigned-64 (first *projection-arguments*))
           2000))
(assert (equal (rest *projection-arguments*)
               '(1 20 8044 420 4000 56 72 104 210 480 #xffffaf)))
(assert (retrodeck:configure-text-projection
         600000000 1 20 8044 420 4000 56 72 104 210 480 #xffffaf))
(assert (= (decode-native-unsigned-64 (first *projection-arguments*))
           600000000))
(assert (retrodeck:draw-projected-text 17 44))
(assert (equal *projected-text-arguments* '(17 44)))
(assert (retrodeck:clear-text-mask-cache))
(assert (= *text-mask-clear-count* 1))
(setf *projection-status* 0
      *projected-text-status* 0)
(assert (not (retrodeck:configure-text-projection
              0 1 20 4044 420 4000 56 72 104 210 480 0)))
(assert (not (retrodeck:draw-projected-text 17 0)))
(setf *projection-status* 1
      *projected-text-status* 1)
(dolist (function
         (list (lambda () (retrodeck:load-text-mask "" 0))
               (lambda ()
                 (retrodeck:configure-text-projection
                  -1 1 20 4044 420 4000 56 72 104 210 480 0))
               (lambda () (retrodeck:draw-projected-text 0 0))))
  (assert-signals type-error (funcall function)))

(setf *raster-cover-result* 17)
(assert-native-call (retrodeck:load-cover-raster #P"/tmp/cover.png" #x5f87ff)
                    17 *raster-cover-arguments* '("/tmp/cover.png" #x5f87ff))
(setf *raster-png-result* 18)
(assert-native-call (retrodeck:load-png-raster "/tmp/icon.png" 23 23)
                    18 *raster-png-arguments* '("/tmp/icon.png" 23 23))
(assert-native-call (retrodeck:draw-canvas-raster 18 -4 8 50 50)
                    t *canvas-raster-arguments* '(18 -4 8 50 50))
(setf *canvas-raster-status* 0)
(assert (not (retrodeck:draw-canvas-raster 18 0 0 1 1)))
(setf *canvas-raster-status* 1
      *raster-cover-result* 0
      *raster-png-result* 0)
(dolist (function (list (lambda () (retrodeck:load-cover-raster "/tmp/x" #x1000000))
                        (lambda () (retrodeck:load-png-raster "/tmp/x" 0 1))
                        (lambda () (retrodeck:load-png-raster "/tmp/x" 2049 1))
                        (lambda () (retrodeck:draw-canvas-raster 0 0 0 1 1))))
  (assert-signals type-error (funcall function)))

(assert (equal retrodeck:*chiptune-extensions*
               '(".ay" ".gbs" ".gym" ".hes" ".kss" ".nsf" ".nsfe" ".sap"
                 ".spc" ".vgm" ".vgz" ".ogg")))
(assert (and (= retrodeck:*chiptune-maximum-depth* 4)
             (= retrodeck:*chiptune-maximum-files* 1024)
             (= retrodeck:*chiptune-maximum-file-size* 16777216)))
(assert-binary-table #'eq #'retrodeck:chiptune-file-accepted-p
                     `(("demo.NSFE" 16777216 t) ("demo.ogg" 1 t)
                       ("demo.mp3" 1 nil) ("demo.ogg" 0 nil)
                       ("demo.ogg" 16777217 nil) ("demo" 1 nil)
                       (,(format nil "demo.~CSS" (code-char #x212a)) 1 nil)))
(flet ((lister-for (tree)
         (lambda (path)
           (cdr (assoc path tree :test #'string=)))))
  (let ((tree
          '(("/music"
             (:name "zelda.nsf" :kind :file :size 4096)
             (:name ".hidden.ogg" :kind :file :size 4096)
             (:name "broken.ogg" :kind :other :size 0)
             (:name "huge.spc" :kind :file :size 16777217)
             (:name "empty.gbs" :kind :file :size 0)
             (:name "readme.txt" :kind :file :size 12)
             (:name "b-side" :kind :directory :size 0)
             (:name "a-side" :kind :directory :size 0))
            ("/music/a-side"
             (:name "crazy.OGG" :kind :file :size 100))
            ("/music/b-side"
             (:name "deep" :kind :directory :size 0))
            ("/music/b-side/deep"
             (:name "tune.ay" :kind :file :size 1)))))
    (assert (equal (retrodeck:scan-chiptune-files
                    "/music" :lister (lister-for tree))
                   '("/music/a-side/crazy.OGG"
                     "/music/b-side/deep/tune.ay"
                     "/music/zelda.nsf"))))
  (let ((tree '(("/deep" (:name "d" :kind :directory :size 0))
                ("/deep/d" (:name "d" :kind :directory :size 0))
                ("/deep/d/d" (:name "d" :kind :directory :size 0))
                ("/deep/d/d/d" (:name "d" :kind :directory :size 0))
                ("/deep/d/d/d/d"
                 (:name "reachable.ogg" :kind :file :size 1)
                 (:name "d" :kind :directory :size 0))
                ("/deep/d/d/d/d/d"
                 (:name "unreachable.ogg" :kind :file :size 1)))))
    (assert (equal (retrodeck:scan-chiptune-files
                    "/deep" :lister (lister-for tree))
                   '("/deep/d/d/d/d/reachable.ogg"))))
  (let ((retrodeck:*chiptune-maximum-files* 2))
    (assert (equal (retrodeck:scan-chiptune-files
                    "/cap"
                    :lister (lister-for
                             '(("/cap"
                                (:name "c.ogg" :kind :file :size 1)
                                (:name "a.ogg" :kind :file :size 1)
                                (:name "b.ogg" :kind :file :size 1)))))
                   '("/cap/a.ogg" "/cap/b.ogg")))))
(assert-signals type-error (retrodeck:scan-chiptune-files 4))
(assert-binary-table #'string= #'retrodeck:chiptune-display-text
                     '(("a_z 9.:+-/Č!" 64 "A Z 9.:+-/")
                       ("abcdef" 6 "ABCDEF") ("abcdef" 5 "AB...")
                       ("abcdef" 3 "ABC") ("abcdef" 0 "")))
(assert (string= (retrodeck:chiptune-display-text
                  (format nil "x~Cy" #\Tab) 3) "X Y"))
(assert-unary-table #'string= #'retrodeck:chiptune-base-name
                    '(("/tmp/my-song_name.ogg" "my song name")
                      ("archive.tar.NSF" "archive.tar")
                      (".hidden" "") ("plain" "plain")))
(assert-unary-table #'string= #'retrodeck:chiptune-format-time
                    '((-1 "0:00") (0 "0:00") (999 "0:00") (1000 "0:01")
                      (59999 "0:59") (60000 "1:00") (3600000 "60:00")))
(assert-unary-table #'string= #'retrodeck:main-invocation-name
                    '(("/mnt/data/nes-deck/chiptune-deck" "chiptune-deck")
                      ("deck-menu" "deck-menu")
                      ("" "")))
(flet ((route (arguments)
         (multiple-value-list (retrodeck:main-route arguments))))
  (assert (equal (route '("/mnt/data/nes-deck/ten-seconds-deck"))
                 '(:ten-seconds nil)))
  (assert (equal (route '("/mnt/data/nes-deck/chiptune-deck"
                          "/mnt/data/chiptunes"))
                 '(:chiptunes ("/mnt/data/chiptunes"))))
  (assert (equal (route '("./deck-menu")) '(:dashboard nil)))
  (assert (equal (route '("/usr/bin/retrodeck-native" "startup.lisp"))
                 '(:startup ("startup.lisp"))))
  (assert (equal (route nil) '(:startup nil))))
(assert-signals type-error (retrodeck:main-route "deck-menu"))
(let ((*error-output* (make-broadcast-stream)))
  (assert (= (retrodeck:run-ten-seconds-main '("extra")) 2))
  (assert (= (retrodeck:run-chiptune-main nil) 2))
  (assert (= (retrodeck:run-chiptune-main '("/a" "/b")) 2)))
(let ((*program-arguments-result* '("/usr/bin/retrodeck-native"))
      (*standard-output* (make-broadcast-stream)))
  (assert (zerop (retrodeck:main))))
(assert (equal (retrodeck:dashboard-main-options
                '("--manifest" "/tmp/games.tsv" "--palette" "/tmp/palette.tsv"
                  "--chip8-emulator" "/bin/false"))
               '(:manifest "/tmp/games.tsv" :palette "/tmp/palette.tsv"
                 :chip8-emulator "/bin/false")))
(assert (null (retrodeck:dashboard-main-options nil)))
(assert-signals error (retrodeck:dashboard-main-options '("--manifest")))
(assert-signals error (retrodeck:dashboard-main-options '("manifest" "x")))
(assert-signals error (retrodeck:run-dashboard-main '("--manifest" "/tmp/x")))
(let ((*standard-output* (make-broadcast-stream)))
  (assert (zerop (retrodeck:run-dashboard-main '("--help")))))
(assert-unary-table #'equal #'retrodeck:chiptune-control-commands
                    '(((:kind :keyboard :code 28 :shift nil :repeat nil) nil)
                      ((:kind :gamepad :edges #x001) (:back))
                      ((:kind :gamepad :edges #x002) (:back))
                      ((:kind :gamepad :edges #x004) (:toggle-pause))
                      ((:kind :gamepad :edges #x008) (:toggle-pause))
                      ((:kind :gamepad :edges #x010) (:previous-track))
                      ((:kind :gamepad :edges #x020) (:next-track))
                      ((:kind :gamepad :edges #x040) nil)
                      ((:kind :gamepad :edges #x080) (:playback-mode))
                      ((:kind :gamepad :edges #x100) (:previous-file))
                      ((:kind :gamepad :edges #x200) (:next-file))
                      ((:kind :gamepad :edges #x400) (:volume-up))
                      ((:kind :gamepad :edges #x800) (:volume-down))
                      ((:kind :gamepad :edges #x00c) (:toggle-pause))
                      ((:kind :gamepad :edges #x300)
                       (:previous-file :next-file))))
(assert-signals error (retrodeck:chiptune-control-commands '(:kind :mouse)))
(assert-unary-table #'eq #'retrodeck:chiptune-touch-command
                    '((:close :back) (:pause :toggle-pause)
                      (:previous-file :previous-file)
                      (:next-file :next-file)
                      (:playback-mode :playback-mode)))
(assert-signals error (retrodeck:chiptune-touch-command :volume-up))
(assert-unary-table #'eq #'retrodeck:chiptune-next-playback-mode
                    '((:loop-all :loop-one) (:loop-one :shuffle)
                      (:shuffle :loop-all)))
(let ((state 12345))
  (assert (= (setf state (retrodeck:chiptune-next-random state)) 3336926330))
  (assert (= (setf state (retrodeck:chiptune-next-random state)) 1697253807))
  (assert (= (retrodeck:chiptune-next-random state) 2816511904)))
(assert (= (retrodeck:chiptune-next-random 0) 1085196063))
(assert (equal (retrodeck:chiptune-file-candidates 4 1 1) '(2 3 0 1)))
(assert (equal (retrodeck:chiptune-file-candidates 4 1 -1) '(0 3 2 1)))
(assert (equal (retrodeck:chiptune-file-candidates 1 0 1) '(0)))
(assert (equal (retrodeck:chiptune-shuffle-candidates 4 1 6) '(2 3 0)))
(assert (equal (retrodeck:chiptune-shuffle-candidates 4 1 7) '(3 0 2)))
(assert (equal (retrodeck:chiptune-shuffle-candidates 1 0 99) '(0)))
(assert-unary-table
 #'equal (lambda (arguments)
           (apply #'retrodeck:chiptune-advance-plan arguments))
 '(((:loop-one 0 1) (:restart)) ((:loop-one 2 4) (:restart))
   ((:shuffle 0 1) (:shuffle)) ((:loop-all 0 3) (:track 1))
   ((:loop-all 2 3) (:next-file)) ((:loop-all 0 1) (:next-file))))
(assert-binary-table #'= #'retrodeck:chiptune-volume-step
                     '((42 1 47) (98 1 100) (100 1 100)
                       (42 -1 37) (4 -1 0) (0 -1 0)))
(assert-signals type-error (retrodeck:chiptune-volume-step 101 1))
(assert-signals type-error (retrodeck:chiptune-next-random -1))
(assert-signals type-error (retrodeck:chiptune-file-accepted-p 4 1))
(assert-signals type-error (retrodeck:chiptune-file-accepted-p "x.ogg" 1.5))
(assert-signals type-error (retrodeck:chiptune-display-text "x" -1))
(assert-signals type-error (retrodeck:chiptune-base-name 4))
(assert-signals type-error (retrodeck:chiptune-format-time 1.5))

(assert (= (retrodeck::chiptune-color :orange) #xff6d20))
(assert (= (retrodeck::chiptune-color :muted) #x949594))
(assert-unary-table #'eq (lambda (point)
                           (apply #'retrodeck:chiptune-touch-action point))
                    '(((1124 22) :close) ((1247 89) :close)
                      ((446 370) :previous-file) ((650 370) :pause)
                      ((854 370) :next-file) ((242 370) :playback-mode)
                      ((1248 22) nil) ((1124 90) nil)))
(let ((pcm (make-string 2940 :initial-element (code-char 0))))
  (flet ((sample (index value)
           (let ((unsigned (ldb (byte 16 0) value))
                 (offset (* index 2)))
             (setf (char pcm offset) (code-char (ldb (byte 8 0) unsigned))
                   (char pcm (1+ offset))
                   (code-char (ldb (byte 8 8) unsigned))))))
    (sample 0 -32768)
    (sample 1 32767)
    (sample 1469 -1))
  (let ((*chiptune-open-result* '("" "artist" "" "" 50851 1))
        (*chiptune-step-result* (list pcm 0 735 66))
        (*chiptune-open-path* nil)
        (*chiptune-start-track-index* nil))
    (assert (equal (retrodeck:open-chiptune-file "/tmp/crazy.ogg")
                   '(:title "crazy" :subtitle "artist" :system "OGG VORBIS"
                     :length 50851 :track-index 0 :track-count 1)))
    (assert (and (typep *chiptune-open-path* 'base-string)
                 (string= *chiptune-open-path* "/tmp/crazy.ogg")))
    (let ((*chiptune-open-result*
            '("Song 3" "Game" "Author" "Nintendo NES" 150000 4)))
      (assert (equal (retrodeck:open-chiptune-file "/tmp/game.nsf")
                     '(:title "Song 3" :subtitle "Game - Author"
                       :system "Nintendo NES" :length 150000
                       :track-index 0 :track-count 4)))
      (assert (equal (retrodeck:start-chiptune-track "/tmp/game.nsf" 2)
                     '(:title "Song 3" :subtitle "Game - Author"
                       :system "Nintendo NES" :length 150000
                       :track-index 2 :track-count 4)))
      (assert (= *chiptune-start-track-index* 2))
      (assert-signals error (retrodeck:start-chiptune-track "/tmp/game.nsf" 4)))
    (let ((*chiptune-open-result* '("" "" "COMPOSER" "Spectrum" -1 2)))
      (assert (equal (retrodeck:open-chiptune-file "/tmp/tune.ay")
                     '(:title "tune" :subtitle "COMPOSER" :system "Spectrum"
                       :length -1 :track-index 0 :track-count 2))))
    (multiple-value-bind (raw-pcm ended frames position)
        (retrodeck:step-chiptune-file)
      (assert (and (string= raw-pcm pcm)
                   (not ended) (= frames 735) (= position 66)))
      (let ((visual (retrodeck::chiptune-decode-pcm raw-pcm)))
        (assert (and (typep visual '(simple-array (signed-byte 16) (1470)))
                     (= (aref visual 0) -32768)
                     (= (aref visual 1) 32767)
                     (zerop (aref visual 2))
                     (= (aref visual 1469) -1)))))
    (assert (and (retrodeck:open-chiptune-audio 42)
                 (= *chiptune-audio-open-volume* 42)
                 (retrodeck:write-chiptune-audio pcm)
                 (string= *chiptune-audio-write-pcm* pcm)
                 (retrodeck:close-chiptune-audio)
                 (retrodeck:rewind-chiptune-file)
                 (retrodeck:close-chiptune-file)))))
(let ((*chiptune-open-result* nil) (*chiptune-step-result* nil)
      (*chiptune-rewind-status* 0) (*chiptune-close-status* 0)
      (*chiptune-audio-open-status* 0) (*chiptune-audio-write-status* 0)
      (*chiptune-audio-close-status* 0))
  (assert (and (null (retrodeck:open-chiptune-file "/tmp/missing.ogg"))
               (null (retrodeck:step-chiptune-file))
               (not (retrodeck:open-chiptune-audio 42))
               (not (retrodeck:write-chiptune-audio
                     (make-string 2940 :initial-element (code-char 0))))
               (not (retrodeck:close-chiptune-audio))
               (not (retrodeck:rewind-chiptune-file))
               (not (retrodeck:close-chiptune-file)))))
(let ((*chiptune-open-result* '("bad"))
      (*chiptune-step-result* '("short" 0 735 0)))
  (assert-signals error (retrodeck:open-chiptune-file "/tmp/bad.ogg"))
  (assert-signals error (retrodeck:start-chiptune-track "/tmp/bad.nsf" 0))
  (assert-signals error (retrodeck:step-chiptune-file))
  (assert-signals error (retrodeck:open-chiptune-audio 101))
  (assert-signals error (retrodeck:write-chiptune-audio "short")))
(let ((ready (retrodeck:make-chiptune-render-state
              :ready t :title "crazy" :system "ogg vorbis"
              :position 25000 :length 50000 :file-index 1 :file-count 3
              :track-index 0 :track-count 1 :volume 42)))
  (setf *canvas-fill-calls* nil *canvas-glyph-calls* nil)
  (assert (retrodeck:render-chiptune ready))
  (assert (and (zerop *canvas-clear-color*)
               (member '(16 16 1248 448 0) *canvas-fill-calls* :test #'equal)
               (member '(208 184 864 88 0) *canvas-fill-calls* :test #'equal)
               (member '(208 226 864 2 9737620) *canvas-fill-calls* :test #'equal)
               (member '(208 284 432 6 8629891) *canvas-fill-calls* :test #'equal)
               (member '(582 116 67 4 16777215) *canvas-glyph-calls*
                       :test #'equal)
               (member '(32 44 86 2 8629891) *canvas-glyph-calls*
                       :test #'equal))))
(let ((visual (make-array 1470 :element-type '(signed-byte 16)
                          :initial-element 0)))
  (setf (aref visual 0) 32767 (aref visual 1) 32767
        (aref visual 1466) -32768 (aref visual 1467) -32768
        *canvas-fill-calls* nil)
  (assert (retrodeck:render-chiptune
           (retrodeck:make-chiptune-render-state
            :ready t :title "x" :system "ogg" :file-count 1 :track-count 1
            :visual visual)))
  (assert (and (member '(208 186 2 40 16739616) *canvas-fill-calls*
                       :test #'equal)
               (member '(1070 228 2 40 16739616) *canvas-fill-calls*
                       :test #'equal))))
(dolist (fixture '((:loop-one t (330 398 49 2 16777215))
                   (:shuffle nil (310 394 2 2 16777215))))
  (destructuring-bind (mode paused expected) fixture
    (setf *canvas-fill-calls* nil *canvas-glyph-calls* nil)
    (assert (retrodeck:render-chiptune
             (retrodeck:make-chiptune-render-state
              :ready nil :playback-mode mode :paused paused :volume 0)))
    (assert (member expected (if (eq mode :loop-one)
                                 *canvas-glyph-calls* *canvas-fill-calls*)
                    :test #'equal))))
(dolist (function
         (list (lambda () (retrodeck:make-chiptune-render-state
                           :ready t :file-count 0 :track-count 1))
               (lambda () (retrodeck:make-chiptune-render-state
                           :ready nil :playback-mode :random))
               (lambda () (retrodeck:make-chiptune-render-state
                           :ready nil :visual #(0)))
               (lambda () (retrodeck:chiptune-touch-action 1.5 2))))
  (assert-signals error (funcall function)))

(assert (string= (retrodeck:display-ascii "AČz") "A?z"))
(labels ((bytes (&rest values)
           (map 'string #'code-char values)))
  (assert (string= (retrodeck::display-utf8-bytes-ascii
                    (bytes #xc5 #xbd #x6c #x75 #xc5 #xa5) 4)
                   "?lu?"))
  (dolist (invalid (list (bytes #xc0 #xaf)
                         (bytes #x80)
                         (bytes #xc5)
                         (bytes #xe2 #x28 #xa1)
                         (bytes #xed #xa0 #x80)
                         (bytes #x1f)
                         "Č"))
    (assert-signals error
                    (retrodeck::display-utf8-bytes-ascii invalid 64)))
  (assert-signals error
                  (retrodeck::display-utf8-bytes-ascii "AB" 1)))
(assert (= (retrodeck:bitmap-text-width "" 2) 0))
(assert (= (retrodeck:bitmap-text-width "AB" 2) 22))
(assert (= (retrodeck:fit-text-scale "ABCDE" 29 3 1) 1))
(assert (= (retrodeck:fit-text-scale "ABCDE" 1 3 2) 2))
(assert (string= (retrodeck:fit-text-width "ABCDEFGHIJ" 29 1) "AB..."))
(assert (string= (retrodeck:fit-text-width "AB" 1 1) ""))

(assert-recorded-calls *canvas-glyph-calls*
    (retrodeck:draw-text 100 100 "AČ" 2 #xeeeeee)
    '((100 100 65 2 #xeeeeee) (112 100 63 2 #xeeeeee)))
(assert-recorded-calls *canvas-glyph-calls*
    (retrodeck:draw-centered-text 10 20 30 40 "AB" 2 #xeeeeee)
    '((14 33 65 2 #xeeeeee) (26 33 66 2 #xeeeeee)))
(assert-recorded-calls *canvas-fill-calls*
    (retrodeck:stroke-canvas-rect 10 20 30 40 2 #xeeeeee)
    '((10 20 30 2 #xeeeeee) (10 58 30 2 #xeeeeee)
      (10 20 2 40 #xeeeeee) (38 20 2 40 #xeeeeee)))
(assert-recorded-calls *canvas-fill-calls*
    (retrodeck:fill-pixel-cut-rect 100 100 20 12 4 #xfe6c27)
    '((104 100 12 12 #xfe6c27) (100 104 20 4 #xfe6c27)))
(assert-recorded-calls *canvas-fill-calls*
    (retrodeck:fill-pixel-cut-rect 100 100 8 12 4 #xfe6c27) nil)
(assert-recorded-calls *canvas-fill-calls*
    (retrodeck:draw-pixel-panel 100 100 20 20 #x121212 #xfe6c27 4)
    '((104 100 12 20 #xfe6c27) (100 104 20 12 #xfe6c27)
      (108 104 4 12 #x121212) (104 108 12 4 #x121212)))

(assert-unary-table #'string= #'retrodeck:ten-seconds-format
                    '((0 "00.00") (123 "01.23") (10000 "99.99")))
(assert-unary-table
 #'equal #'retrodeck:ten-seconds-cue-notes
 '((:start ((523 28) (784 38))) (:exact ((784 35) (1047 40) (1319 55)))
   (:miss ((659 35) (440 55)))))
(dolist (fixture '((:back ((16 16) (167 16) (16 79) (167 79)))
                   (:touch ((15 16) (168 16) (16 15) (16 80)))))
  (destructuring-bind (expected points) fixture
    (dolist (point points)
      (assert (eq (apply #'retrodeck:ten-seconds-touch-event point) expected)))))
(let ((ready (retrodeck:ten-seconds-initial-state)))
  (assert-ten-seconds-reduction ready :back 0 '((:exit)) :identity ready))
(dolist (fixture '((:touch 999 "09.99" :miss)
                   (:controller-a 1000 "10.00" :exact)
                   (:touch 1001 "10.01" :miss)
                   (:touch 10000 "99.99" :miss)))
  (destructuring-bind (input elapsed expected cue) fixture
    (let* ((running (assert-ten-seconds-reduction
                     (retrodeck:ten-seconds-initial-state) input 100
                     '((:cue :start) (:redraw)) :mode :running))
           (stopped (assert-ten-seconds-reduction
                     running input (+ 100 (* elapsed 10000000))
                     (list (list :result expected :input input) (list :cue cue)
                           '(:redraw))
                     :mode :stopped :displayed (min elapsed 9999)))
           (retry-at (+ 101 (* elapsed 10000000))))
      (assert-ten-seconds-reduction stopped input retry-at
                                    '((:cue :start) (:redraw))
                                    :mode :running :started-at retry-at))))
(let ((running (nth-value 0 (retrodeck:ten-seconds-reduce
                             (retrodeck:ten-seconds-initial-state) :touch 100))))
  (dolist (fixture `((99 nil :identity ,running)
                     (100 ((:redraw)) :displayed 0 :redraw-at 33000100)
                     (1230000100 ((:redraw)) :displayed 123
                      :redraw-at 1263000100)))
    (apply #'assert-ten-seconds-reduction running :tick fixture)))
(setf *canvas-fill-calls* nil *canvas-glyph-calls* nil)
(assert (retrodeck:render-ten-seconds
         (retrodeck:ten-seconds-initial-state)))
(assert (and (zerop *canvas-clear-color*)
             (equal (car (last *canvas-fill-calls*))
                    '(16 16 1248 448 #x100d0c))
             (equal (car (last *canvas-glyph-calls*))
                    '(46 38 66 4 #xffedc2))))

(dolist (fixture '((nil nil nil) ("layer-shell" nil nil)
                   (nil "wayland-1" nil) ("widget" "wayland-1" nil)
                   ("layer-shell" "" nil) ("layer-shell" "wayland-1" t)))
  (destructuring-bind (presentation display expected) fixture
    (assert (eq (retrodeck::ten-seconds-wayland-requested-p
                 presentation display) expected))))
(let ((runtime (retrodeck:make-ten-seconds-runtime
                :presentation "layer-shell" :wayland-display "wayland-1")))
  (assert (and (getf runtime :wayland) (null (getf runtime :volume))
               (getf runtime :dirty) (not (getf runtime :auto-presentation))
               (eq (getf runtime :clock) #'retrodeck:monotonic-nanoseconds))))
(let ((errors (make-string-output-stream)))
  (let ((*error-output* errors))
    (assert (and (= (retrodeck::ten-seconds-runtime-volume nil) 42)
                 (zerop (retrodeck::ten-seconds-runtime-volume "bad")))))
  (assert (search "game cues disabled" (get-output-stream-string errors))))
(multiple-value-bind (state trace summary)
    (exercise-ten-seconds-runtime nil '((200 200 1 1 0)) '(100 101))
  (assert-plist-values state '(:mode :running :started-at 101))
  (assert (equal trace
                 '((:reap-sound) (:tick :now 100) (:render) (:present)
                   (:poll :wayland nil :timeout 8) (:controls 0) (:touches 1)
                   (:touch :now 101) (:cue :start) (:redraw))))
  (assert-plist-values summary
                       '(:owned t :play (((523 28) (784 38)) 42) :stops 1)))
(multiple-value-bind (state trace summary)
    (exercise-ten-seconds-runtime '((1 4 0))
                                  '((200 200 1 1 0) (20 20 1 1 0)) '(200))
  (assert-plist-values state '(:mode :ready))
  (assert (equal (last trace 2) '((:back :now 200) (:exit))))
  (assert-plist-values summary '(:play nil)))
(multiple-value-bind (state trace summary)
    (exercise-ten-seconds-runtime
     '((0 28 0) (1 8 0) (1 4 0))
     '((200 200 1 1 0) (201 201 1 1 0)) '(300 301) :active 1 :play 2)
  (assert-plist-values state '(:started-at 301))
  (assert (find :controller-a trace :key #'first))
  (assert (not (find :touch trace :key #'first)))
  (assert-plist-values summary '(:owned nil)))
(multiple-value-bind (state trace ignored)
    (exercise-ten-seconds-runtime
     '((0 28 0) (1 8 0))
     '((200 200 1 1 0) (201 201 1 1 0) (201 201 0 0 1))
     '(500 501 1000000501) :active 1 :play 2)
  (declare (ignore ignored))
  (assert-plist-values state '(:mode :stopped :displayed 100))
  (assert (equal (remove-if-not (lambda (item) (eq (first item) :touch)) trace)
                 '((:touch :now 501) (:touch :now 1000000501)))))
(multiple-value-bind (state trace summary)
    (exercise-ten-seconds-runtime nil nil '(600) :wayland t)
  (declare (ignore state trace))
  (assert-plist-values summary '(:poll (0 8))))
(let ((runtime (retrodeck:make-ten-seconds-runtime
                :presentation nil :wayland-display nil))
      (diagnostics (make-string-output-stream)))
  (with-runtime-device-fixture (:gamepads-scan-result '(-1 "cannot scan gamepads"))
    (let ((*error-output* diagnostics))
      (retrodeck:ten-seconds-runtime-initialize runtime))
    (let* ((text (get-output-stream-string diagnostics))
           (failure (search "controller input unavailable: cannot scan gamepads" text))
           (ready (search "0 THEGamepad controller(s) ready" text)))
      (assert (and failure ready (< failure ready) (= (getf runtime :volume) 42)
                   (not (search "game cues disabled" text)))))
    (retrodeck:ten-seconds-runtime-shutdown runtime)
    (assert (and (= *evdev-gamepads-scan-count* 1)
                 (zerop *evdev-controls-scan-count*)))))

(with-ten-seconds-runtime-fixture
    (runtime (:presentation "layer-shell" :wayland-display "wayland-test")
             (:wayland-open-status 0))
  (assert-signals error
    (retrodeck:ten-seconds-candidate-rehearse
     (retrodeck:ten-seconds-initial-state) runtime :iteration-limit 1))
  (assert-runtime-observations :wayland-open 1 :wayland-kind :gameplay :fbdev-open 0
   :wayland-close 0 :evdev-close 1 :controls-close 1 :initialized nil))

(with-ten-seconds-runtime-fixture
    (runtime (:presentation nil :wayland-display nil
              :clock (lambda () (error "Clock ran after shutdown")))
             () (state (retrodeck:ten-seconds-initial-state)))
  (let ((*process-shutdown-status* 1) (*active-count* 0))
    (retrodeck:ten-seconds-runtime-initialize runtime)
    (multiple-value-bind (returned ignored trace)
        (retrodeck:ten-seconds-runtime-run-iteration state runtime)
      (declare (ignore ignored))
      (assert (eq returned state))
      (assert (equal trace '((:shutdown))))
      (assert-runtime-observations :active-count 0 :input-poll nil))
    (retrodeck:ten-seconds-runtime-shutdown runtime)))

(with-ten-seconds-runtime-fixture
    (runtime (:presentation nil :wayland-display nil
              :clock (lambda () (pop times))) ()
             (times '(100 101 102)) (*active-status* 1) (*play-status* 1)
             (*stop-count* 0) (*input-poll-result* '(1 0 1 0 0 0 0))
             (*canvas-clear-status* 1)
             (retrodeck::*menu-sound-input-until-ms* 777))
  (let ((*evdev-touch-queue* '((200 200 1 1 0)))
          (*record-interaction* :cleanup-error))
    (let ((failure
            (handler-case
                (progn
                  (retrodeck:ten-seconds-candidate-rehearse
                   (retrodeck:ten-seconds-initial-state) runtime :iteration-limit 2
                   :stop-predicate (lambda (state ignored iteration)
                                     (declare (ignore state ignored))
                                     (when (= iteration 1)
                                       (setf *canvas-clear-status* 0))
                                     nil))
                  nil)
              (error (condition) condition))))
      (assert (search "10 Seconds render failed" (princ-to-string failure))))
    (assert-runtime-observations :stop-count 1 :fbdev-close 1 :evdev-close 1 :controls-close 1
     :menu-sound-until 777 :initialized nil :dirty nil)
    (assert (not (getf runtime :running)))))

(dolist (fixture (list (list 0 nil :limit 0)
                       (list 1 (constantly t) :operator-stop 0)
                       (list 1 nil :limit 1)))
  (destructuring-bind (limit stop expected trace-count) fixture
    (with-ten-seconds-runtime-fixture (runtime (:wayland-display "") ())
      (multiple-value-bind (state returned traces reason)
          (retrodeck:ten-seconds-candidate-rehearse
           (retrodeck:ten-seconds-initial-state) runtime
           :iteration-limit limit :stop-predicate stop)
        (declare (ignore state))
        (assert (eq returned runtime))
        (assert (= (length traces) trace-count))
        (assert (eq reason expected))
        (assert-runtime-observations :initialized nil :fbdev-close 1)))))

(defun exercise-chiptune-runtime
    (controls touches times
     &key (entries '(("crazy.ogg" 0 100)))
       (open-result '("" "artist" "" "" 50851 1))
       step-result (iterations 1) volume-state (audio-open 1)
       (write-status 1))
  (let* ((runtime (retrodeck:make-chiptune-runtime
                   :directory "/tunes"
                   :presentation nil :wayland-display nil
                   :volume-state (or volume-state "")
                   :clock (lambda () (or (pop times) 999))))
         (*list-directory-result* entries)
         (*list-directory-path* nil)
         (*chiptune-open-result* open-result)
         (*chiptune-open-path* nil)
         (*chiptune-start-track-index* nil)
         (*chiptune-step-result* step-result)
         (*chiptune-audio-open-status* audio-open)
         (*chiptune-audio-open-volume* nil)
         (*chiptune-audio-write-status* write-status)
         (*chiptune-audio-write-pcm* nil)
         (*chiptune-rewind-status* 1)
         (*chiptune-rewind-count* 0)
         (*chiptune-close-status* 1)
         (*chiptune-audio-close-status* 1)
         (*input-poll-result* (list 1 (length controls) (length touches) 0 1 0 0))
         (*canvas-clear-status* 1)
         (*canvas-glyph-status* 1)
         (*canvas-fill-status* 1)
         (*state-file-write-arguments* nil))
    (with-runtime-device-fixture (:controls controls)
      (let ((*error-output* (make-broadcast-stream))
            (*evdev-touch-queue* touches))
        (let ((state (retrodeck:chiptune-runtime-initialize runtime))
              (traces nil))
          (dotimes (index iterations)
            (when (getf runtime :running)
              (multiple-value-bind (next ignored trace)
                  (retrodeck:chiptune-runtime-run-iteration state runtime)
                (declare (ignore ignored))
                (setf state next)
                (push trace traces))))
          (let ((summary (list :volume (getf runtime :volume)
                               :running (getf runtime :running)
                               :blocks (getf runtime :chiptune-blocks)
                               :open-path *chiptune-open-path*
                               :audio-volume *chiptune-audio-open-volume*
                               :written *chiptune-audio-write-pcm*
                               :state-write *state-file-write-arguments*
                               :start-track *chiptune-start-track-index*
                               :rewinds *chiptune-rewind-count*)))
            (retrodeck:chiptune-runtime-shutdown runtime)
            (values state (nreverse traces) summary)))))))

(let ((pcm (make-string 2940 :initial-element (code-char 0))))
  ;; One happy iteration: scan, open, decode a block, feed audio, present.
  (multiple-value-bind (state traces summary)
      (exercise-chiptune-runtime nil nil (list 1 2 3)
                                 :step-result (list pcm 0 735 66))
    (assert-plist-values state '(:file-index 0 :position 66 :paused nil
                                 :playback-mode :loop-all))
    (assert (equal (getf state :files) '("/tunes/crazy.ogg")))
    (assert (string= (getf (getf state :metadata) :title) "crazy"))
    (assert (string= (getf state :visual) pcm))
    (assert (equal (first traces)
                   '((:generate :now 3 :decoded t) (:render) (:present)
                     (:poll :timeout 0 :controls 0 :touches 0))))
    (assert-plist-values summary '(:running t :blocks 1 :audio-volume 42))
    (assert (string= (getf summary :written) pcm)))
  ;; An empty library renders the guidance screen and close exits.
  (multiple-value-bind (state traces summary)
      (exercise-chiptune-runtime nil '((1130 30 1 1 0)) (list 1 2 3)
                                 :entries nil)
    (assert (null (getf state :metadata)))
    (assert (string= (getf state :status) "ADD MUSIC TO /tunes"))
    (assert (equal (first (last (first traces))) '(:commands (:back))))
    (assert-plist-values summary '(:running nil :blocks 0 :open-path nil)))
  ;; Gamepad edges drive navigation, volume, and persistence.
  (multiple-value-bind (state traces summary)
      (exercise-chiptune-runtime '((1 #x200 0) (1 #x400 0)) nil (list 1 2 3)
                                 :entries '(("a.ogg" 0 10) ("b.ogg" 0 10))
                                 :volume-state "/tmp/volume.state")
    (declare (ignore traces))
    (assert-plist-values state '(:file-index 1))
    (assert-plist-values summary '(:volume 47 :audio-volume 47))
    (assert (equal (getf summary :open-path) "/tunes/b.ogg"))
    (assert (equal (getf summary :state-write)
                   (list "/tmp/volume.state" (format nil "47~%")))))
  ;; Touch cycles the playback mode; loop-one track ends rewind in place.
  (multiple-value-bind (state traces summary)
      (exercise-chiptune-runtime nil '((250 380 1 1 0)) (list 1 2 3)
                                 :step-result (list pcm 0 735 66))
    (declare (ignore traces summary))
    (assert-plist-values state '(:playback-mode :loop-one)))
  (multiple-value-bind (state traces summary)
      (exercise-chiptune-runtime nil nil (list 1 2 3 4)
                                 :entries '(("a.ogg" 0 10) ("b.ogg" 0 10))
                                 :step-result (list pcm 1 735 50851)
                                 :iterations 1)
    (declare (ignore traces))
    ;; loop-all with a single-track file advances to the next file.
    (assert-plist-values state '(:file-index 1))
    (assert (equal (getf summary :open-path) "/tunes/b.ogg"))
    (assert-plist-values summary '(:rewinds 0))))

(let* ((credits-path (test-menu-path "credits.tsv"))
       (*regular-file-result* (test-file-string credits-path))
       (credits (retrodeck:load-project-credits credits-path))
       (crawl (retrodeck:make-project-credits-crawl credits)))
  (assert (= (length credits) 30))
  (assert (string= (getf (first credits) :project) "FCEUmm"))
  (assert (string= (getf (car (last credits)) :project)
                   "OpenGameArt contributors"))
  (assert (= (length (getf crawl :lines)) 95))
  (assert (= (length (getf crawl :static-lines)) 30))
  (assert (= (getf crawl :content-height) 5076))
  (assert (equal (first (getf crawl :lines))
                 '(:text "RETRO DECK" :source-y 0
                   :source-width 236 :source-height 28)))
  (assert (equal (car (last (getf crawl :lines)))
                 '(:text "THANK YOU" :source-y 5032
                   :source-width 212 :source-height 28)))
  (assert (every (lambda (line)
                   (and (<= (getf line :source-width) 1040)
                        (= (getf line :source-height) 28)))
                 (getf crawl :lines)))

  (assert (retrodeck:clear-credits-text-mask-cache))
  (setf *text-mask-result* 17
        *text-mask-calls* nil
        *projected-text-calls* nil
        *projection-arguments* nil
        *canvas-fill-calls* nil
        *canvas-glyph-calls* nil)
  (assert (equal (retrodeck:render-project-credits crawl nil 2000)
                 '(:close (1212 12 56 56))))
  (assert (= *canvas-clear-color* #x000000))
  (assert (= (decode-native-unsigned-64 (first *projection-arguments*))
             2000))
  (assert (equal (rest *projection-arguments*)
                 '(1 20 9076 420 4000 56 72 104 210 480 #xffffac)))
  (let ((projected (nreverse *projected-text-calls*)))
    (assert (= (length projected) 95))
    (assert (equal (first projected) '(17 0)))
    (assert (equal (car (last projected)) '(17 5032))))
  (assert (= (length *canvas-fill-calls*) 96))
  (assert (null *canvas-glyph-calls*))
  (assert (<= (length *text-mask-calls*) 101))
  (assert (> (length *text-mask-calls*) 70))

  (setf *canvas-fill-calls* nil
        *canvas-glyph-calls* nil
        *projected-text-calls* nil
        *projection-arguments* nil)
  (retrodeck:render-project-credits crawl t 0)
  (let ((first-fills (reverse *canvas-fill-calls*))
        (first-glyphs (reverse *canvas-glyph-calls*)))
    (assert (= (length first-fills) 14))
    (assert (null *projected-text-calls*))
    (assert (null *projection-arguments*))
    (assert (member '(20 20 70 2 #xffffac) first-glyphs :test #'equal))
    (assert (member '(20 458 47 1 #x949594) first-glyphs :test #'equal))
    (setf *canvas-fill-calls* nil
          *canvas-glyph-calls* nil)
    (retrodeck:render-project-credits crawl t 60000)
    (assert (equal (reverse *canvas-fill-calls*) first-fills))
    (assert (equal (reverse *canvas-glyph-calls*) first-glyphs)))

  (let ((layout '(:close (1212 12 56 56))))
    (assert (eq (retrodeck:credits-target-at layout 1212 12) :close))
    (assert (eq (retrodeck:credits-target-at layout 1267 67) :close))
    (assert (null (retrodeck:credits-target-at layout 1268 67)))
    (assert-touch-release
     retrodeck:credits-touch-transition
     (retrodeck:credits-initial-state) layout
     '(1240 40 t t nil) '(1240 40 nil nil t)
     (assert (null (getf released :pressed-target)))
     (assert (equal release-effect '(:close t :cue :back)))))

  (setf *regular-file-result* nil)
  (assert-signals error (retrodeck:load-project-credits "/tmp/missing.tsv"))
  (assert-signals error (retrodeck:load-project-credits "relative.tsv"))
  (dolist (contents
           (list "# only a comment\n"
                 "bad\trow\n"
                 "same\trole\tMIT\nsame\trole\tMIT\n"
                 (format nil "~A\trole\tMIT\n" (make-string 49
                                                          :initial-element #\A))
                 (with-output-to-string (output)
                   (dotimes (index 65)
                     (format output "project-~D\trole\tMIT~%" index)))))
    (setf *regular-file-result* contents)
    (assert-signals error
                    (retrodeck:load-project-credits "/tmp/invalid.tsv"))))

(let* ((network '(:ssid "net1" :wlan-ipv4 "10.249.110.248"
                  :wireguard-ipv4 "10.0.0.10" :selector "CONNECTED"))
       (*canvas-fill-calls* nil)
       (*canvas-glyph-calls* nil)
       (layout (retrodeck:render-dashboard-settings
                42 60 "us" :volume-down "" network)))
  (assert (= *canvas-clear-color* #x000000))
  (assert (equal layout
                 '(:close (1212 12 56 56)
                   :wifi (926 20 262 108)
                   :volume-down (108 208 104 104)
                   :volume-up (228 208 104 104)
                   :brightness-down (438 208 104 104)
                   :brightness-up (558 208 104 104)
                   :terminal (792 208 112 104)
                   :keymap (1036 208 112 104))))
  (assert (member '(112 208 96 104 #xfe6c27)
                  *canvas-fill-calls* :test #'equal))
  (assert (member '(116 212 88 96 #x503311)
                  *canvas-fill-calls* :test #'equal))
  (assert (member '(232 208 96 104 #x6c6c6c)
                  *canvas-fill-calls* :test #'equal))
  (assert (member '(236 212 88 96 #x303030)
                  *canvas-fill-calls* :test #'equal))
  (assert (member '(64 44 110 3 #xeeeeee)
                  *canvas-glyph-calls* :test #'equal))
  (assert (member '(832 247 62 2 #xeeeeee)
                  *canvas-glyph-calls* :test #'equal))
  (dolist (target '(:close :wifi :volume-down :volume-up
                    :brightness-down :brightness-up :terminal :keymap))
    (assert-rect-target-boundaries
     #'retrodeck:settings-target-at layout target))
  (setf *canvas-fill-calls* nil
        *canvas-glyph-calls* nil)
  (retrodeck:render-dashboard-settings 0 60 "us" :volume-up "" network)
  (assert (member '(194 334 79 3 #xeeeeee)
                  *canvas-glyph-calls* :test #'equal))
  (setf *canvas-glyph-calls* nil)
  (retrodeck:render-dashboard-settings 42 60 "cz" :keymap "" network)
  (assert (member '(1070 246 67 4 #xeeeeee)
                  *canvas-glyph-calls* :test #'equal))
  (setf *canvas-glyph-calls* nil)
  (retrodeck:render-dashboard-settings
   42 100 "us" :brightness-up "BRIGHTNESS 100%" network)
  (assert (member '(551 447 66 2 #xbcbcbc)
                  *canvas-glyph-calls* :test #'equal)))

(assert (string= (retrodeck:dashboard-settings-label :active-wifi)
                 "ACTIVE WIFI"))
(assert (equal (retrodeck:dashboard-settings-geometry :close)
               '(1212 12 56 56)))
(assert (string= (retrodeck:dashboard-settings-path :volume-state)
                 "/mnt/data/nes-deck/state/menu-volume.state"))
(assert (= (retrodeck:settings-volume-after-target :volume-down 42 42) 37))
(assert (= (retrodeck:settings-volume-after-target :volume-down 5 42) 0))
(assert (= (retrodeck:settings-volume-after-target :volume-up 0 42) 42))
(assert (= (retrodeck:settings-volume-after-target :volume-up 0 0) 5))
(assert (= (retrodeck:settings-volume-after-target :volume-up 100 42) 100))
(assert (= (retrodeck:settings-brightness-after-target :brightness-down 10) 10))
(assert (= (retrodeck:settings-brightness-after-target :brightness-down 60) 50))
(assert (= (retrodeck:settings-brightness-after-target :brightness-up 100) 100))

(let ((state (retrodeck:settings-initial-state
              :volume 42 :brightness 60 :keymap "us")))
  (assert (equal state
                 '(:open t :volume 42 :last-audible-volume 42
                   :brightness 60 :keymap "us" :selected :volume-down
                   :pressed-target nil :status "")))
  (multiple-value-bind (moved effect)
      (retrodeck:settings-move-selection state :previous)
    (assert (eq (getf moved :selected) :wifi))
    (assert (equal effect '(:cue :previous)))
    (multiple-value-bind (wrapped next-effect)
        (retrodeck:settings-move-selection moved :next)
      (assert (eq (getf wrapped :selected) :volume-down))
      (assert (equal next-effect '(:cue :next)))))
  (let ((close-selected (copy-list state)))
    (setf (getf close-selected :selected) :close)
    (multiple-value-bind (next effect)
        (retrodeck:settings-move-selection close-selected :next)
      (assert (eq (getf next :selected) :volume-down))
      (assert (equal effect '(:cue :next))))
    (multiple-value-bind (previous effect)
        (retrodeck:settings-move-selection close-selected :previous)
      (assert (eq (getf previous :selected) :wifi))
      (assert (equal effect '(:cue :previous)))))
  (multiple-value-bind (confirmed plan)
      (retrodeck:settings-controller-transition state :confirm)
    (assert (eq (getf confirmed :selected) :volume-down))
    (assert (eq (getf plan :action) :volume))
    (assert (= (getf plan :value) 37))
    (assert (equal (getf plan :success-effect) '(:cue :volume)))
    (assert (string= (getf plan :success-status) "GAME VOLUME 37%"))
    (assert-settings-completion
     confirmed plan t '(:cue :volume)
     '((= :volume 37) (= :last-audible-volume 37)
       (string= :status "GAME VOLUME 37%")))
    (assert-settings-completion
     confirmed plan t '(:cue :volume)
     '((string= :status "VOLUME SAVED; CONFIRMATION TONE FAILED"))
     :tone-succeeded-p nil)
    (assert-settings-completion
     confirmed plan nil nil
     '((= :volume 42) (string= :status "VOLUME STATE ERROR"))))
  (multiple-value-bind (back back-plan)
      (retrodeck:settings-controller-transition state :back)
    (multiple-value-bind (closed effect)
        (retrodeck:settings-complete-action back back-plan t)
      (assert (not (getf closed :open)))
      (assert (equal effect '(:cue :back)))))
  (let* ((muting-state (retrodeck:settings-initial-state
                        :volume 5 :last-audible-volume 42
                        :brightness 60 :keymap "us"))
         (mute-plan (retrodeck:settings-activation-plan
                     muting-state :volume-down)))
    (assert (zerop (getf mute-plan :value)))
    (assert (equal (getf mute-plan :success-effect) '(:stop-sound t)))
    (let ((muted
            (assert-settings-completion
             muting-state mute-plan t '(:stop-sound t)
             '((zerop :volume) (= :last-audible-volume 42)
               (string= :status "GAME VOLUME MUTED")))))
      (let ((restore-plan (retrodeck:settings-activation-plan
                           muted :volume-up)))
        (assert (= (getf restore-plan :value) 42))
        (assert-settings-completion
         muted restore-plan nil nil
         '((zerop :volume) (string= :status "VOLUME STATE ERROR")))))
    (assert-settings-completion
     muting-state mute-plan nil nil
     '((= :volume 5) (string= :status "VOLUME STATE ERROR"))))
  (let ((brightness-plan
          (retrodeck:settings-activation-plan state :brightness-up)))
    (assert (= (getf brightness-plan :value) 70))
    (assert (eq (getf brightness-plan :cue) :next))
    (assert (string= (getf brightness-plan :device-path)
                     "/sys/class/backlight/display-bl/brightness"))
    (assert-settings-completion
     state brightness-plan t '(:cue :next)
     '((= :brightness 70) (string= :status "BRIGHTNESS 70%")))
    (assert-settings-completion
     state brightness-plan nil '(:cue :next)
     '((= :brightness 60)
       (string= :status "BRIGHTNESS ERROR - CHECK LOG"))))
  (let ((keymap-plan (retrodeck:settings-activation-plan state :keymap)))
    (assert (string= (getf keymap-plan :value) "cz"))
    (assert (eq (getf keymap-plan :cue) :confirm))
    (assert-settings-completion
     state keymap-plan t '(:cue :confirm)
     '((string= :keymap "cz") (string= :status "TERMINAL KEYS: CZECH")))
    (assert-settings-completion
     state keymap-plan nil '(:cue :confirm)
     '((string= :keymap "us") (string= :status "KEYMAP STATE ERROR"))))
  (assert (equal (retrodeck:settings-activation-plan state :terminal)
                 '(:action :terminal :mode "shell" :cue :confirm)))
  (assert (equal (retrodeck:settings-activation-plan state :wifi)
                 '(:action :wifi :cue :confirm))))

(let* ((state (retrodeck:dashboard-loop-initial-state nil :keymap "us"))
       (plan (retrodeck:settings-activation-plan
              (getf state :settings) :keymap)))
  (setf (getf state :pending-settings-plan) plan)
  (multiple-value-bind (failed effects)
      (retrodeck:dashboard-reduce state '(:settings-result :succeeded-p nil))
    (assert (null (getf failed :pending-settings-plan)))
    (assert (string= (getf (getf failed :settings) :keymap) "us"))
    (assert (string= (getf (getf failed :settings) :status)
                     "KEYMAP STATE ERROR"))
    (assert (equal effects '((:render) (:present) (:cue :confirm))))))

(let* ((layout '(:close (1212 12 56 56)
                 :wifi (926 20 262 108)
                 :volume-down (108 208 104 104)
                 :volume-up (228 208 104 104)
                 :brightness-down (438 208 104 104)
                 :brightness-up (558 208 104 104)
                 :terminal (792 208 112 104)
                 :keymap (1036 208 112 104)))
       (state (retrodeck:settings-initial-state
               :volume 42 :brightness 60 :keymap "us")))
  (assert-touch-release retrodeck:settings-touch-transition state layout
                        '(128 228 t t nil) '(128 228 nil nil t)
    (assert (null (getf released :pressed-target)))
    (assert (eq (getf released :selected) :volume-down))
    (assert (= (getf release-effect :value) 37)))
  (assert-touch-release retrodeck:settings-touch-transition state layout
                        '(128 228 t t nil) '(248 228 nil nil t)
    (assert (null release-effect))
    (assert (null (getf released :pressed-target))))
  (assert-touch-release retrodeck:settings-touch-transition state layout
                        '(1240 40 t t nil) '(1240 40 nil nil t)
    (assert (eq (getf release-effect :action) :close))
    (assert (eq (getf release-effect :cue) :back))
    (assert (eq (getf released :selected) :close))))

(let* ((network '(:ssid "net1" :wlan-ipv4 "10.249.110.248"
                  :wireguard-ipv4 "10.0.0.10" :selector "CONNECTED"))
       (state (retrodeck:wifi-initial-state))
       (*canvas-fill-calls* nil)
       (*canvas-glyph-calls* nil)
       (layout (retrodeck:render-dashboard-wifi state network)))
  (assert (= *canvas-clear-color* #x000000))
  (assert (equal (subseq layout 0 16)
                 '(:back (16 10 120 62)
                   :ssid (330 10 310 62)
                   :passphrase (650 10 330 62)
                   :save (990 10 274 62)
                   :mode (16 364 152 66)
                   :shift (176 364 168 66)
                   :space (352 364 700 66)
                   :delete (1060 364 204 66))))
  (let ((keys (getf layout :keys)))
    (assert (= (length keys) 30))
    (assert (equal (first keys) '((18 86 119 62) #\q)))
    (assert (equal (nth 10 keys) '((17 154 133 62) #\a)))
    (assert (equal (car (last keys)) '((956 290 307 62) #\-))))
  (assert (member '(330 10 310 62 #x121212)
                  *canvas-fill-calls* :test #'equal))
  (assert (member '(330 10 310 3 #x87afff)
                  *canvas-fill-calls* :test #'equal))
  (assert (member '(650 10 330 3 #x5f5f5f)
                  *canvas-fill-calls* :test #'equal))
  (dolist (target '(:back :ssid :passphrase :save
                    :mode :shift :space :delete))
    (assert-rect-target-boundaries #'retrodeck:wifi-target-at layout target))
  (assert (equal (retrodeck:wifi-target-at layout 18 86) '(:key 0 #\q)))
  (assert (equal (retrodeck:wifi-target-at layout 136 147)
                 '(:key 0 #\q)))
  (assert (null (retrodeck:wifi-target-at layout 137 147)))
  (let* ((uppercase (retrodeck:wifi-initial-state :uppercase t))
         (uppercase-layout
           (retrodeck:render-dashboard-wifi uppercase network)))
    (assert (= (length (getf uppercase-layout :keys)) 30))
    (assert (equal (first (getf uppercase-layout :keys))
                   '((18 86 119 62) #\Q))))
  (let* ((symbols (retrodeck:wifi-initial-state
                   :ssid "NETWORK" :passphrase "password"
                   :field :passphrase :symbols t))
         (symbols-layout (retrodeck:render-dashboard-wifi symbols network)))
    (assert (= (length (getf symbols-layout :keys)) 42))
    (assert (equal (nth 30 (getf symbols-layout :keys))
                   '((19 290 98 62) #\`)))
    (assert (equal (car (last (getf symbols-layout :keys)))
                   '((1163 290 98 62) #\>)))))

(assert (string= (retrodeck:dashboard-wifi-label :title) "ADD WIFI"))
(assert (equal (retrodeck:dashboard-wifi-geometry :back) '(16 10 120 62)))
(assert (equal (retrodeck:dashboard-wifi-key-rows :alphabet)
               '("qwertyuiop" "asdfghjkl" "zxcvbnm" "@._-")))
(assert (= (retrodeck:dashboard-wifi-limit :passphrase-maximum) 63))
(assert (string= (retrodeck:dashboard-wifi-path :profile-helper)
                 "/usr/sbin/deck-wifi-profile-add"))
(assert (string= (retrodeck:dashboard-wifi-path :selector-status)
                 "/var/run/deck-wifi/status"))
(assert (string= (retrodeck:wifi-tail-for-field "short" 19) "short"))
(assert (string= (retrodeck:wifi-tail-for-field "123456789" 5) "...89"))
(assert (string= (retrodeck:wifi-tail-for-field "123456789" 3) "789"))
(assert (retrodeck:wifi-valid-text-p "test net" 1 32))
(assert (not (retrodeck:wifi-valid-text-p (format nil "bad~%ssid") 1 32)))

(let* ((network '(:ssid "net1" :wlan-ipv4 "10.249.110.248"
                  :wireguard-ipv4 "10.0.0.10" :selector "CONNECTED"))
       (state (retrodeck:wifi-initial-state :status "OLD"))
       (layout (retrodeck:render-dashboard-wifi state network)))
  (assert (equal state
                 '(:open t :ssid "" :passphrase "" :field :ssid
                   :uppercase nil :symbols nil :status "OLD"
                   :pressed-target nil)))
  (let ((opened (retrodeck:wifi-open-state state)))
    (assert (getf opened :open))
    (assert (string= (getf opened :status) "")))
  (multiple-value-bind (keyed applied)
      (retrodeck:wifi-apply-target state layout '(:key 0 #\q))
    (assert applied)
    (assert (string= (getf keyed :ssid) "q"))
    (assert (string= (getf keyed :status) "")))
  (multiple-value-bind (unchanged applied)
      (retrodeck:wifi-apply-target state layout '(:key -1 #\q))
    (assert (not applied))
    (assert (equal unchanged state)))
  (multiple-value-bind (password-field applied)
      (retrodeck:wifi-apply-target state layout :passphrase)
    (assert applied)
    (assert (eq (getf password-field :field) :passphrase))
    (multiple-value-bind (spaced space-applied)
        (retrodeck:wifi-apply-target password-field layout :space)
      (assert space-applied)
      (assert (string= (getf spaced :passphrase) " "))
      (multiple-value-bind (deleted delete-applied)
          (retrodeck:wifi-apply-target spaced layout :delete)
        (assert delete-applied)
        (assert (string= (getf deleted :passphrase) "")))))
  (multiple-value-bind (symbols applied)
      (retrodeck:wifi-apply-target state layout :mode)
    (assert applied)
    (assert (getf symbols :symbols))
    (multiple-value-bind (unchanged shift-applied)
        (retrodeck:wifi-apply-target symbols layout :shift)
      (assert (not shift-applied))
      (assert (not (getf unchanged :uppercase)))
      (assert (string= (getf unchanged :status) ""))))
  (multiple-value-bind (uppercase applied)
      (retrodeck:wifi-apply-target state layout :shift)
    (assert applied)
    (assert (getf uppercase :uppercase)))
  (let ((full (retrodeck:wifi-initial-state
               :ssid (make-string 32 :initial-element #\x) :status "OLD")))
    (multiple-value-bind (limited applied)
        (retrodeck:wifi-apply-target full layout '(:key 0 #\q))
      (assert applied)
      (assert (= (length (getf limited :ssid)) 32))
      (assert (string= (getf limited :status) ""))))
  (multiple-value-bind (ignored effect)
      (retrodeck:wifi-controller-transition state :confirm)
    (assert (equal ignored state))
    (assert (null effect)))
  (multiple-value-bind (closed effect)
      (retrodeck:wifi-controller-transition state :back)
    (assert (not (getf closed :open)))
    (assert (equal effect
                   '(:action :close :dashboard-status "WIFI EDITOR CLOSED"
                     :cue :back))))
  (multiple-value-bind (invalid effect)
      (retrodeck:wifi-activate-target state layout :save)
    (assert (string= (getf invalid :status)
                     "SSID MUST BE 1 TO 32 CHARACTERS"))
    (assert (equal effect '(:cue :confirm))))
  (let ((short-password
          (retrodeck:wifi-initial-state :ssid "test net" :passphrase "short")))
    (multiple-value-bind (invalid effect)
        (retrodeck:wifi-activate-target short-password layout :save)
      (assert (string= (getf invalid :status)
                       "PASSWORD MUST BE 8 TO 63 CHARACTERS"))
      (assert (equal effect '(:cue :confirm)))))
  (let ((valid (retrodeck:wifi-initial-state
                :ssid "test net" :passphrase "secret!9")))
    (multiple-value-bind (plan error-status) (retrodeck:wifi-save-plan valid)
      (assert (null error-status))
      (assert (eq (getf plan :action) :save))
      (assert (string= (getf plan :executable)
                       "/usr/sbin/deck-wifi-profile-add"))
      (assert (string= (getf plan :input) "test net
secret!9
"))
      (multiple-value-bind (saved effect)
          (retrodeck:wifi-complete-save valid plan t)
        (assert (string= (getf saved :passphrase) ""))
        (assert (string= (getf saved :status)
                         "WIFI SAVED - USED AFTER CURRENT WIFI DISCONNECTS"))
        (assert (equal effect '(:cue :confirm))))
      (multiple-value-bind (failed effect)
          (retrodeck:wifi-complete-save
           valid plan nil :failure-status "WIFI PROFILE WRITE FAILED")
        (assert (string= (getf failed :passphrase) "secret!9"))
        (assert (string= (getf failed :status)
                         "WIFI PROFILE WRITE FAILED"))
        (assert (equal effect '(:cue :confirm)))))))

(dolist (fixture
         '(((0 0 -1 nil)
            (:phase :complete :exit-code 0 :signal nil :error nil))
           ((0 7 -1 nil)
            (:phase :complete :exit-code 7 :signal nil :error nil))
           ((0 -1 15 nil)
            (:phase :complete :exit-code nil :signal 15 :error nil))
           ((1 -1 -1 "start")
            (:phase :start :exit-code nil :signal nil :error "start"))
           ((2 0 -1 "pipe")
            (:phase :input :exit-code 0 :signal nil :error "pipe"))
           ((3 -1 -1 "wait")
            (:phase :wait :exit-code nil :signal nil :error "wait"))))
  (assert (equal (retrodeck::decode-native-helper-result (first fixture))
                 (second fixture))))

(dolist (result
         '(() (0 0 -1) (0 0 . -1) (4 0 -1 nil)
           (0 -2 -1 nil) (0 256 -1 nil) (0 0 0 nil)
           (0 -1 -1 nil) (0 0 15 nil) (0 0 -1 "error")
           (1 0 -1 "start") (1 -1 -1 nil)
           (2 0 15 "pipe") (2 0 -1 nil)
           (3 -1 15 "wait") (3 -1 -1 nil)))
  (assert-signals error (retrodeck::decode-native-helper-result result)))

(let* ((wifi (retrodeck:wifi-initial-state
              :ssid "test net" :passphrase "secret!9"))
       (plan (retrodeck:wifi-save-plan wifi))
       (*helper-result* '(0 0 -1 nil))
       (*helper-arguments* nil))
  (assert (equal (retrodeck:run-dashboard-wifi-profile plan)
                 '(:wifi-result :succeeded-p t)))
  (assert (equal *helper-arguments*
                 (list "/usr/sbin/deck-wifi-profile-add"
                       (format nil "test net~%secret!9~%")))))

(flet ((exercise-wifi-helper (native-result)
         (let* ((wifi (retrodeck:wifi-initial-state
                       :ssid "test net" :passphrase "secret!9"))
                (plan (retrodeck:wifi-save-plan wifi))
                (state (retrodeck:dashboard-loop-initial-state
                        nil :wifi-state wifi))
                (runtime (retrodeck:make-dashboard-runtime))
                (*helper-result* native-result)
                (*helper-arguments* nil)
                (diagnostics (make-string-output-stream)))
           (setf (getf state :pending-wifi-plan) plan)
           (let* ((*error-output* diagnostics)
                  (completion
                    (runtime-effect runtime (list :wifi-action plan) state)))
             (multiple-value-bind (next effects)
                 (retrodeck:dashboard-reduce state completion)
               (list completion next effects *helper-arguments*
                     (get-output-stream-string diagnostics)))))))
  (dolist (fixture
           '(((0 0 -1 nil) t
              "WIFI SAVED - USED AFTER CURRENT WIFI DISCONNECTS" "")
             ((0 7 -1 nil) nil "WIFI PROFILE WAS NOT SAVED" "secret!9")
             ((2 0 -1 "pipe") nil "WIFI PROFILE WRITE FAILED" "secret!9")
             ((1 -1 -1 "start") nil "WIFI PROFILE WAS NOT SAVED" "secret!9")
             ((3 -1 -1 "wait") nil "WIFI PROFILE WAS NOT SAVED" "secret!9")
             ((0 -1 15 nil) nil "WIFI PROFILE WAS NOT SAVED" "secret!9")))
    (destructuring-bind (native-result succeeded-p status passphrase) fixture
      (destructuring-bind (completion next effects arguments diagnostics)
          (exercise-wifi-helper native-result)
        (assert (equal completion
                       (if succeeded-p
                           '(:wifi-result :succeeded-p t)
                           (if (= (first native-result) 2)
                               '(:wifi-result :succeeded-p nil
                                 :failure-status "WIFI PROFILE WRITE FAILED")
                               '(:wifi-result :succeeded-p nil)))))
        (assert (string= (getf (getf next :wifi) :status) status))
        (assert (string= (getf (getf next :wifi) :passphrase) passphrase))
        (assert (null (getf next :pending-wifi-plan)))
        (assert (equal effects '((:render) (:cue :confirm) (:present))))
        (assert (equal arguments
                       (list "/usr/sbin/deck-wifi-profile-add"
                             (format nil "test net~%secret!9~%"))))
        (assert (null (search "secret!9" diagnostics)))))))

(let* ((wifi (retrodeck:wifi-initial-state
              :ssid "test net" :passphrase "secret!9"))
       (plan (retrodeck:wifi-save-plan wifi))
       (state (retrodeck:dashboard-loop-initial-state nil :wifi-state wifi))
       (handled nil)
       (runtime
         (retrodeck:make-dashboard-runtime
          :external-effect-handler
          (lambda (effect current)
            (declare (ignore current))
            (setf handled effect)
            '(:wifi-result :succeeded-p nil))))
       (*helper-arguments* nil))
  (assert-runtime-effect runtime (list :wifi-action plan) state
                         '(:wifi-result :succeeded-p nil))
  (assert (equal handled (list :wifi-action plan)))
  (assert (null *helper-arguments*)))

(let ((*helper-arguments* :not-called))
  (dolist (wifi (list (retrodeck:wifi-initial-state
                       :ssid "" :passphrase "secret!9")
                      (retrodeck:wifi-initial-state
                       :ssid "test net" :passphrase "short")))
    (multiple-value-bind (plan error-status) (retrodeck:wifi-save-plan wifi)
      (assert (null plan))
      (assert (stringp error-status))))
  (assert (eq *helper-arguments* :not-called)))

(let* ((network '(:ssid "net1" :wlan-ipv4 "10.249.110.248"
                  :wireguard-ipv4 "10.0.0.10" :selector "CONNECTED"))
       (state (retrodeck:wifi-initial-state
               :ssid "test net" :passphrase "secret!9"))
       (layout (retrodeck:render-dashboard-wifi state network)))
  (assert-touch-release retrodeck:wifi-touch-transition state layout
                        '(18 86 t t nil) '(18 86 nil nil t)
    (assert (string= (getf released :ssid) "test netq"))
    (assert (equal release-effect '(:cue :next))))
  (assert-touch-release retrodeck:wifi-touch-transition state layout
                        '(18 86 t t nil) '(143 86 nil nil t)
    (assert (string= (getf released :ssid) "test net"))
    (assert (null release-effect)))
  (assert-touch-release retrodeck:wifi-touch-transition state layout
                        '(1000 20 t t nil) '(1000 20 nil nil t)
    (assert (eq (getf release-effect :action) :save))
    (assert (string= (getf released :passphrase) "secret!9")))
  (assert-touch-release retrodeck:wifi-touch-transition state layout
                        '(20 20 t t nil) '(20 20 nil nil t)
    (assert (not (getf released :open)))
    (assert (eq (getf release-effect :action) :close))))

(let* ((games '((:id "alpha" :title "ALPHA" :system :nes :color #x5f87ff)
                (:id "beta" :title "BETA" :system :nes :color #xafd75f)
                (:id "long-title" :title "A VERY LONG FIXTURE GAME TITLE"
                 :system :nes :color #xffd700)
                (:id "delta" :title "DELTA" :system :nes :color #xd75f5f)
                (:id "gb" :title "GB FIXTURE" :system :gb :color #x87af87)
                (:id "gbc" :title "GBC FIXTURE" :system :gbc :color #xecb6e7)
                (:id "zx" :title "ZX FIXTURE" :system :zx :color #x87afff)
                (:id "deck-fixture" :title "DECK FIXTURE" :system :deck
                 :color #xff8700)))
       (*canvas-fill-calls* nil)
       (*canvas-glyph-calls* nil)
       (layout (retrodeck:render-dashboard games :nes 2 "FIXTURE STATUS")))
  (assert (= *canvas-clear-color* #x000000))
  (assert (equal (getf layout :systems) '(:nes :gb :gbc :zx :deck)))
  (assert (equal (getf layout :system-buttons)
                 '((56 76 227 52) (291 76 227 52) (526 76 227 52)
                   (761 76 227 52) (996 76 227 52))))
  (assert (equal (getf layout :game-indices) '(0 1 2 3)))
  (assert (= (getf layout :shown-game-index) 2))
  (assert (equal (getf layout :visible-game-indices) '(1 2 3)))
  (assert (equal (getf layout :game-buttons)
                 '((280 154 216 264) (532 154 216 264) (784 154 216 264))))
  (assert (equal (getf layout :indicators)
                 '((596 438 16 8) (620 438 16 8)
                   (644 438 16 8) (668 438 16 8))))
  (assert (equal (getf layout :previous) '(156 232 80 100)))
  (assert (equal (getf layout :next) '(1044 232 80 100)))
  (assert (member '(536 154 208 264 #xfe6c27) *canvas-fill-calls*
                  :test #'equal))
  (assert (member '(536 162 208 248 #x503311) *canvas-fill-calls*
                  :test #'equal))
  (assert (member '(557 457 70 2 #xbcbcbc) *canvas-glyph-calls*
                  :test #'equal)))

(let* ((games '((:id "only-nes" :title "ONLY NES" :system :nes
                 :color #x5f87ff)))
       (layout (retrodeck:render-dashboard games :gb 0 "NO MATCH")))
  (assert (= (getf layout :shown-game-index) (length games)))
  (assert (null (getf layout :game-indices)))
  (assert (null (getf layout :visible-game-indices)))
  (assert (null (getf layout :game-buttons)))
  (assert (null (getf layout :indicators)))
  (assert (equal (getf layout :previous) '(156 232 80 100)))
  (assert (equal (getf layout :next) '(1044 232 80 100))))

(let* ((games '((:id "nes" :title "NES" :system :nes :color #x5f87ff)
                (:id "gb" :title "ONLY GB" :system :gb :color #x87af87)))
       (layout (retrodeck:render-dashboard games :gb 7 "ONE CARD")))
  (assert (= (getf layout :shown-game-index) 1))
  (assert (equal (getf layout :game-indices) '(1)))
  (assert (equal (getf layout :visible-game-indices) '(1)))
  (assert (equal (getf layout :game-buttons) '((532 154 216 264))))
  (assert (equal (getf layout :indicators) '((632 438 16 8))))
  (assert (equal (getf layout :previous) '(0 0 0 0)))
  (assert (equal (getf layout :next) '(0 0 0 0))))

(assert (retrodeck:clear-dashboard-raster-cache))
(assert (= *raster-clear-count* 1))
(setf *raster-png-result* 23
      *raster-png-calls* nil
      *canvas-raster-calls* nil)
(let ((retrodeck:*dashboard-settings-icon-path* "/tmp/settings.png"))
  (retrodeck:render-dashboard nil :nes 0 "")
  (assert (equal *raster-png-arguments* '("/tmp/settings.png" 23 23)))
  (assert (= (length *raster-png-calls*) 1))
  (assert (member '(23 1215 415 50 50) *canvas-raster-calls*
                  :test #'equal)))

(assert (retrodeck:clear-dashboard-raster-cache))
(assert (= *raster-clear-count* 2))
(setf *raster-png-result* 0
      *raster-cover-result* 24
      *raster-cover-calls* nil
      *canvas-fill-calls* nil
      *canvas-raster-calls* nil)
(let* ((games '((:id "covered" :title "COVERED" :system :nes
                 :color #x5f87ff :cover "/tmp/fixture.png")))
       (layout (retrodeck:render-dashboard games :nes 0 "COVERED")))
  (assert (= (getf layout :shown-game-index) 0))
  (assert (equal *raster-cover-arguments*
                 '("/tmp/fixture.png" #x5f87ff)))
  (assert (= (length *raster-cover-calls*) 1))
  (assert (member '(24 540 162 200 200) *canvas-raster-calls*
                  :test #'equal))
  (assert (not (member '(578 190 124 144 #x5f87ff) *canvas-fill-calls*
                       :test #'equal))))
(setf *raster-cover-result* 0)

(setf *evdev-controls-scan-result* '(2 3)
      *evdev-controls-dispatch-result* '(2 1)
      *evdev-controls* '((0 15 1)))
(assert (equal (retrodeck:scan-evdev-controls)
               '(:gamepads 2 :keyboards 3)))
(assert (equal (retrodeck:dispatch-evdev-controls 25)
               '(:count 2 :rescan t)))
(assert (= *evdev-controls-dispatch-timeout* 25))
(assert (equal (retrodeck:next-evdev-control)
               '(:kind :keyboard :code 15 :shift t :repeat nil)))
(setf *evdev-controls* '((1 #x501 0)))
(assert (equal (retrodeck:next-evdev-control)
               '(:kind :gamepad :edges #x501)))
(setf *evdev-controls* nil)
(assert (null (retrodeck:next-evdev-control)))
(assert (retrodeck:close-evdev-controls))
(assert (= *evdev-controls-close-count* 1))
(setf *evdev-controls-scan-result* '(3 0))
(assert-signals error (retrodeck:scan-evdev-controls))
(setf *evdev-controls-scan-result* nil
      *evdev-controls-dispatch-result* nil)
(assert (null (retrodeck:scan-evdev-controls)))
(assert (null (retrodeck:dispatch-evdev-controls)))
(assert-signals type-error (retrodeck:dispatch-evdev-controls #x100000000))

(setf *input-poll-result* '(1 2 3 1 1 0 0))
(assert (equal (retrodeck:poll-native-input nil 25)
               '(:poll-ready-p t :control-count 2 :touch-count 3
                 :touch-lost-p t :rescan-controls-p t :shutdown-p nil
                 :refresh-p nil)))
(assert (equal *input-poll-arguments* '(0 25)))
(setf *input-poll-result* '(0 0 0 0 0 1 1))
(assert (equal (retrodeck:poll-native-input t 0)
               '(:poll-ready-p nil :control-count 0 :touch-count 0
                 :touch-lost-p nil :rescan-controls-p nil :shutdown-p t
                 :refresh-p t)))
(assert (equal *input-poll-arguments* '(1 0)))
(dolist (invalid '((0 1 0 0 0 0 0)
                   (0 0 0 1 0 0 0)
                   (2 0 0 0 0 0 0)
                   (1 65 0 0 0 0 0)
                   (1 0 -1 0 0 0 0)
                   (1 0 0 0 0 2)
                   (1 0 0 0 0 0)))
  (setf *input-poll-result* invalid)
  (assert-signals error (retrodeck:poll-native-input nil 0)))
(setf *input-poll-result* nil)
(assert (null (retrodeck:poll-native-input nil 0)))
(assert-signals type-error (retrodeck:poll-native-input nil #x100000000))
(setf *input-poll-result* '(0 0 0 0 0 0 0))

(assert (equal retrodeck:*dashboard-keyboard-controls*
               '((1 :back)
                 (15 :system-next :system-previous)
                 (28 :confirm)
                 (96 :confirm)
                 (103 :up)
                 (105 :left)
                 (106 :right)
                 (108 :down))))
(assert (equal retrodeck:*dashboard-gamepad-controls*
               '((#x001 . :back)
                 (#x002 . :back)
                 (#x004 . :confirm)
                 (#x008 . :confirm)
                 (#x010 . :system-previous)
                 (#x020 . :system-next)
                 (#x040 . :settings)
                 (#x100 . :left)
                 (#x200 . :right)
                 (#x400 . :up)
                 (#x800 . :down))))
(dolist (fixture '((1 nil :back)
                   (15 nil :system-next)
                   (15 t :system-previous)
                   (28 nil :confirm)
                   (96 nil :confirm)
                   (103 nil :up)
                   (105 nil :left)
                   (106 nil :right)
                   (108 nil :down)))
  (destructuring-bind (code shift expected) fixture
    (assert (equal (retrodeck:dashboard-control-actions
                    (list :kind :keyboard :code code
                          :shift shift :repeat nil))
                   (list expected)))))
(dolist (definition retrodeck:*dashboard-gamepad-controls*)
  (assert (equal (retrodeck:dashboard-control-actions
                  (list :kind :gamepad :edges (car definition)))
                 (list (cdr definition)))))
(assert (null (retrodeck:dashboard-control-actions
               '(:kind :gamepad :edges #x080))))
(assert (equal (retrodeck:dashboard-control-actions
                '(:kind :keyboard :code 28 :shift nil :repeat nil))
               '(:confirm)))
(assert (equal (retrodeck:dashboard-control-actions
                '(:kind :keyboard :code 15 :shift nil :repeat nil))
               '(:system-next)))
(assert (equal (retrodeck:dashboard-control-actions
                '(:kind :keyboard :code 15 :shift t :repeat nil))
               '(:system-previous)))
(assert (null (retrodeck:dashboard-control-actions
               '(:kind :keyboard :code 30 :shift nil :repeat nil))))
(assert (equal (retrodeck:dashboard-control-actions
                '(:kind :gamepad :edges #x905))
               '(:back :confirm :left :down)))
(setf *evdev-controls* '((0 106 2) (0 15 1) (1 #x224 0)))
(multiple-value-bind (gamepad keyboard)
    (retrodeck:collect-dashboard-control-actions)
  (assert (and (subsetp gamepad '(:confirm :system-next :right))
               (subsetp '(:confirm :system-next :right) gamepad)))
  (assert (and (subsetp keyboard '(:right :system-previous))
               (subsetp '(:right :system-previous) keyboard))))
(assert (null *evdev-controls*))

(assert-unary-table
 #'eq #'retrodeck:dashboard-controller-command
 '(((:back) nil) ((:settings :system-previous :confirm) :settings)
   ((:system-previous :system-next) :system-previous)
   ((:left :right :confirm) :previous) ((:down :confirm) :next)
   ((:confirm) :confirm)) nil nil)
(assert-unary-table #'eq #'retrodeck:dashboard-controller-command
                    '(((:back :settings) :back)
                      ((:settings :confirm) nil)) t nil)
(assert-unary-table #'eq #'retrodeck:dashboard-controller-command
                    '(((:back :confirm) :back) ((:system-next) nil)) nil t)

(assert-binary-table #'eql #'retrodeck:dashboard-controller-scan-due-p
                     '((nil 0 t) (0 999 t) (1 999 nil) (1 1001 t)))
(assert (retrodeck:dashboard-controller-scan-due-p 900 901 :force t))
(assert (retrodeck:dashboard-controller-scan-due-p 900 901 :rescan t))

(let ((guard (retrodeck:dashboard-controller-guard-initial-state)))
  (dotimes (index 12)
    (multiple-value-bind (next accepted suspended)
        (retrodeck:dashboard-controller-guard-accept-edge guard (* index 50))
      (assert accepted)
      (assert (not suspended))
      (setf guard next)))
  (multiple-value-bind (next accepted suspended)
      (retrodeck:dashboard-controller-guard-accept-edge guard 600)
    (assert (not accepted))
    (assert suspended)
    (assert (getf next :suspended))
    (setf guard next))
  (multiple-value-bind (next accepted suspended)
      (retrodeck:dashboard-controller-guard-accept-edge guard 650)
    (assert (not accepted))
    (assert (not suspended))
    (assert (= (getf next :last-edge-at) 650))
    (setf guard next))
  (multiple-value-bind (next recovered)
      (retrodeck:dashboard-controller-guard-recover-if-quiet guard 1649)
    (assert (not recovered))
    (assert (getf next :suspended)))
  (multiple-value-bind (next recovered)
      (retrodeck:dashboard-controller-guard-recover-if-quiet guard 1650)
    (assert recovered)
    (assert (equal next
                   '(:edge-times nil :suspended nil :last-edge-at nil)))))

(let ((guard (retrodeck:dashboard-controller-guard-initial-state)))
  (multiple-value-bind (next accepted suspended)
      (retrodeck:dashboard-controller-guard-accept-edge guard 0)
    (declare (ignore suspended))
    (assert accepted)
    (setf guard next))
  (multiple-value-bind (next accepted suspended)
      (retrodeck:dashboard-controller-guard-accept-edge guard 1000)
    (declare (ignore suspended))
    (assert accepted)
    (assert (equal (getf next :edge-times) '(1000)))))

(setf *active-status* 0
      retrodeck::*menu-sound-input-until-ms* 0)
(let ((guard (retrodeck:dashboard-controller-guard-initial-state)))
  (multiple-value-bind (actions next suspended)
      (retrodeck:dashboard-controller-input-actions
       '(:confirm) '(:right) guard 100)
    (assert (and (member :confirm actions) (member :right actions)))
    (assert (not suspended))
    (assert (equal (getf next :edge-times) '(100)))
    (setf guard next))
  (setf *active-status* 1)
  (multiple-value-bind (actions next suspended)
      (retrodeck:dashboard-controller-input-actions
       '(:confirm) '(:right) guard 150)
    (assert (equal actions '(:right)))
    (assert (not suspended))
    (assert (equal (getf next :edge-times) '(100 150)))))
(setf *active-status* 0)

(setf *evdev-touch* '(17 23 1 1 0)
      *evdev-dispatch-result* 3)
(assert (retrodeck:open-evdev-touch))
(assert (= (retrodeck:dispatch-evdev-touch 25) 3))
(assert (= *evdev-dispatch-timeout* 25))
(assert (equal (retrodeck:next-evdev-touch) '(17 23 t t nil)))
(setf *evdev-touch* nil
      *evdev-dispatch-result* -1)
(assert (null (retrodeck:next-evdev-touch)))
(assert (null (retrodeck:dispatch-evdev-touch)))
(assert (retrodeck:close-evdev-touch))
(assert (= *evdev-close-count* 1))

(setf *fbdev-size* '(1280 480))
(assert (retrodeck:open-fbdev))
(assert (equal (retrodeck:current-fbdev-size) '(1280 480)))
(assert (retrodeck:present-fbdev-canvas))
(assert (retrodeck:present-fbdev-solid #xfe6c27))
(assert (= *fbdev-present-color* #xfe6c27))
(assert (retrodeck:close-fbdev))
(assert (= *fbdev-close-count* 1))

(assert (retrodeck:open-wayland-widget))
(assert (retrodeck:close-wayland))
(assert (= *wayland-close-count* 1))
(assert (retrodeck:present-wayland-canvas))
(assert (retrodeck:present-wayland-solid #x123456))
(assert (= *wayland-present-color* #x123456))

(setf *wayland-dispatch-result* 4)
(assert (= (retrodeck:dispatch-wayland 25) 4))
(assert (= *wayland-dispatch-timeout* 25))
(setf *wayland-dispatch-result* -1)
(assert (null (retrodeck:dispatch-wayland)))

(setf *wayland-touch* '(1279 0 1 0 0))
(assert (equal (retrodeck:next-wayland-touch)
               '(1279 0 t nil nil)))
(setf *wayland-touch* nil
      *wayland-size* '(1280 480))
(assert (null (retrodeck:next-wayland-touch)))
(assert (equal (retrodeck:current-wayland-size) '(1280 480)))
(assert (not (retrodeck:wayland-shutdown-requested-p)))
(setf *wayland-shutdown-status* 1)
(assert (retrodeck:wayland-shutdown-requested-p))

(assert (equal retrodeck:*dashboard-systems*
               '((:nes "NES")
                 (:gb "GAME BOY")
                 (:gbc "GBC")
                 (:zx "ZX SPECTRUM")
                 (:deck "DECK"))))
(assert (string= (retrodeck:dashboard-system-label :gbc) "GBC"))
(assert (string= (retrodeck:dashboard-system-label :other) "other"))
(assert (string= (retrodeck:dashboard-system-label "MiXeD") "MiXeD"))
(assert (equal retrodeck:*dashboard-palette*
               '((:background . #x000000)
                 (:text-dark . #x121212)
                 (:field . #x121212)
                 (:surface . #x1c1c1c)
                 (:inactive-border . #x5f5f5f)
                 (:control-border . #x6c6c6c)
                 (:footer . #xbcbcbc)
                 (:inactive-text . #xdadada)
                 (:text . #xeeeeee)
                 (:white . #xffffff)
                 (:title . #xffffaf)
                 (:volume-off . #xaf8787)
                 (:volume-on . #x87af87)
                 (:selected . #xecb6e7)
                 (:wifi-active . #x5f87af)
                 (:wifi-focus . #x87afff)
                 (:wifi-active-border . #xafafff)
                 (:field-label . #xafafaf)
                 (:accent . #xfe6c27)
                 (:active . #x503311)
                 (:control-surface . #x303030)
                 (:muted . #x949494))))
  (assert (= (retrodeck:dashboard-color :accent) #xfe6c27))
(assert (equal retrodeck:*dashboard-executables*
               '((:nes . "/mnt/data/nes-deck/nes-deck")
                 (:gb . "/mnt/data/nes-deck/gb-deck")
                 (:zx . "/mnt/data/nes-deck/zx-deck")
                 (:deck . "/mnt/data/nes-deck/ten-seconds-deck")
                 (:chiptunes . "/mnt/data/nes-deck/chiptune-deck")
                 (:terminal . "/mnt/data/nes-deck/terminal/retro-terminal")
                 (:reboot . "/sbin/reboot"))))
(assert (string= retrodeck:*dashboard-cover-directory*
                 "/mnt/data/nes-deck/covers/"))
(assert (string= retrodeck:*dashboard-settings-icon-path*
                 "/mnt/data/nes-deck/menu/settings-icon.png"))
(assert (equal retrodeck:*dashboard-timings*
               '((:child-touch-exit-ms . 2000)
                 (:child-term-grace-ms . 4000)
                 (:reboot-confirm-ms . 4000)
                 (:controller-burst-window-ms . 1000)
                 (:controller-quiet-reset-ms . 1000)
                 (:controller-scan-ms . 1000)
                 (:touch-reconnect-ms . 1000)
                 (:main-poll-ms . 250)
                 (:animated-poll-ms . 40)
                 (:network-refresh-ms . 2000)
                 (:console-mirror-ms . 100))))
(assert (= (retrodeck:dashboard-timing :reboot-confirm-ms) 4000))
(assert (= retrodeck:*dashboard-volume-default* 42))
(assert (= retrodeck:*dashboard-volume-step* 5))
(assert (= retrodeck:*dashboard-brightness-minimum* 10))
(assert (= retrodeck:*dashboard-brightness-step* 10))
(assert (= retrodeck:*dashboard-controller-burst-limit* 12))
(assert (string= retrodeck:*dashboard-reboot-confirmation-text*
                 "PRESS A OR TAP AGAIN TO REBOOT"))
(assert (string= retrodeck:*dashboard-terminal-login-shell* "/BIN/ASH"))
(assert (string= retrodeck:*dashboard-reduced-motion-environment*
                 "RETRO_DECK_REDUCED_MOTION"))
(assert (string= retrodeck:*dashboard-wayland-display-environment*
                 "WAYLAND_DISPLAY"))

(assert (equal retrodeck:*dashboard-built-in-applications*
               '((:id "lua-repl"
                  :title "LUA REPL"
                  :system :deck
                  :rom "/mnt/data/nes-deck/terminal/retro-terminal"
                  :color #x5f87ff
                  :terminal-mode "lua")
                 (:id "lisp-repl"
                  :title "LISP REPL"
                  :system :deck
                  :rom "/mnt/data/nes-deck/terminal/retro-terminal"
                  :color #xafd75f
                  :terminal-mode "lisp")
                 (:id "python-repl"
                  :title "PYTHON REPL"
                  :system :deck
                  :rom "/mnt/data/nes-deck/terminal/retro-terminal"
                  :color #xffd700
                  :terminal-mode "python")
                 (:id "scheme-repl"
                  :title "SCHEME REPL"
                  :system :deck
                  :rom "/mnt/data/nes-deck/terminal/retro-terminal"
                  :color #x87d787
                  :terminal-mode "scheme")
                 (:id "chiptunes"
                  :title "CHIPTUNES"
                  :system :deck
                  :rom "/mnt/data/chiptunes"
                  :color #xff8700)
                 (:id "terminal"
                  :title "TERMINAL"
                  :system :deck
                  :rom "/mnt/data/nes-deck/terminal/retro-terminal"
                  :color #x5f87af
                  :terminal-mode "shell")
                 (:id "reboot"
                  :title "REBOOT"
                  :system :deck
                  :rom "/sbin/reboot"
                  :color #xd75f5f))))
(labels ((bytes (&rest values)
           (map 'string #'code-char values))
         (tsv (&rest fields)
           (with-output-to-string (output)
             (loop for field in fields
                   for separator = nil then t
                   when separator do (write-char #\Tab output)
                   do (write-string field output))
             (write-char #\Return output)
             (terpri output)))
         (game-row (id title system rom &optional (color "#D78787"))
           (tsv id title system rom color))
         (reject-games (contents)
           (let ((*regular-file-result* nil)
                 (*regular-file-results*
                   (and contents (list (cons "/tmp/games.tsv" contents)))))
             (signals-p error
               (retrodeck:load-dashboard-games "/tmp/games.tsv"))))
         (palette-text (entries &optional legacy-icon)
           (with-output-to-string (output)
             (when legacy-icon
               (write-string (tsv "settings-icon" legacy-icon) output))
             (dolist (entry entries)
               (write-string
                (tsv (string-downcase (symbol-name (car entry)))
                     (format nil "#~6,'0X" (cdr entry)))
                output))))
         (reject-palette (contents)
           (let ((*regular-file-result* nil)
                 (*regular-file-results*
                   (and contents (list (cons "/tmp/palette.tsv" contents)))))
             (signals-p error
               (retrodeck:load-dashboard-palette "/tmp/palette.tsv")))))
  (let* ((manifest-path (test-menu-path "games.tsv" t))
         (palette-path (test-menu-path "palette.tsv" t))
         (credits-path (test-menu-path "credits.tsv" t))
         (*regular-file-result* nil)
         (*regular-file-results*
           (list (cons manifest-path (test-file-string manifest-path))
                 (cons palette-path (test-file-string palette-path))
                 (cons credits-path (test-file-string credits-path))))
         (*regular-file-calls* nil))
    (multiple-value-bind (games palette loaded-p)
        (retrodeck:load-dashboard-bootstrap manifest-path palette-path)
      (assert loaded-p)
      (assert
       (equal (mapcar (lambda (game) (getf game :id)) (subseq games 0 13))
              '("mario" "micro-mages" "kirbys-adventure" "metroid" "tetris"
                "pokemon-red" "final-fantasy-legend-iii" "kirbys-dream-land"
                "donkey-kong-country" "super-mario-bros-deluxe" "elite"
                "knight-lore" "ten-seconds")))
      (assert
       (equal (mapcar (lambda (game) (getf game :id)) (subseq games 13))
              '("lua-repl" "lisp-repl" "python-repl" "scheme-repl"
                "chiptunes" "terminal" "reboot")))
      (assert (equal palette retrodeck:*dashboard-palette*)))
    (assert
     (equal (reverse *regular-file-calls*)
            (list (list manifest-path 1 65536)
                  (list palette-path 1 4096))))
    (setf *regular-file-calls* nil)
    (let ((retrodeck:*dashboard-reduced-motion-environment* "PATH")
          (retrodeck:*dashboard-wayland-display-environment* "PATH"))
      (multiple-value-bind
            (state runtime palette palette-loaded-p credits-loaded-p)
          (retrodeck:load-dashboard-candidate-session
           manifest-path palette-path :credits-path credits-path)
        (assert palette-loaded-p)
        (assert credits-loaded-p)
        (assert (= (length (getf state :games)) 20))
        (assert (getf state :reduced-motion))
        (assert (= (length (getf (getf state :credits-crawl) :static-lines))
                   30))
        (assert (getf runtime :auto-presentation))
        (assert (string= (getf runtime :wayland-display)
                         (retrodeck::dashboard-environment-value "PATH")))
        (assert (not (getf runtime :wayland)))
        (assert (equal palette retrodeck:*dashboard-palette*))))
    (assert
     (equal (reverse *regular-file-calls*)
            (list (list manifest-path 1 65536)
                  (list palette-path 1 4096)
                  (list credits-path 1 32768))))
    (let ((retrodeck:*dashboard-reduced-motion-environment*
            "RETRODECK_TEST_REDUCED_MOTION_MUST_BE_MISSING"))
      (assert (not (retrodeck::dashboard-reduced-motion-requested-p))))
    (setf *regular-file-calls* nil)
    (assert-signals error
                    (retrodeck:load-dashboard-candidate-session
                     manifest-path palette-path :credits-path credits-path :runtime nil))
    (assert (null *regular-file-calls*))
    (setf *regular-file-calls* nil)
    (let ((*regular-file-results*
            (list (cons manifest-path (test-file-string manifest-path))
                  (cons palette-path (test-file-string palette-path))))
          (retrodeck:*dashboard-reduced-motion-environment*
            "RETRODECK_TEST_REDUCED_MOTION_MUST_BE_MISSING")
          (errors (make-string-output-stream)))
      (let ((*error-output* errors))
        (multiple-value-bind
              (state runtime ignored-palette palette-loaded-p credits-loaded-p)
            (retrodeck:load-dashboard-candidate-session
             manifest-path palette-path :credits-path credits-path)
          (declare (ignore ignored-palette))
          (assert palette-loaded-p)
          (assert (not credits-loaded-p))
          (assert (not (getf state :reduced-motion)))
          (assert (null (getf (getf state :credits-crawl) :lines)))
          (assert (zerop (getf (getf state :credits-crawl) :content-height)))
          (assert (getf runtime :auto-presentation))))
      (assert (search "the FOSS credits screen will show an error"
                      (get-output-stream-string errors))))
    (assert
     (equal (reverse *regular-file-calls*)
            (list (list manifest-path 1 65536)
                  (list palette-path 1 4096)
                  (list credits-path 1 32768)))))

  (let* ((title (bytes #xc5 #xbd #x6c #x75 #xc5 #xa5))
         (contents
           (concatenate
            'string
            (format nil "# combined launcher manifest~C~%~C~%" #\Return #\Return)
            (tsv "id" "title" "system" "rom" "#RRGGBB")
            (game-row "base-one" "BASE ONE" "nes" "/missing/base.nes")
            (game-row "upload-one" title "gb" "/missing/upload.gb"
                      "#87AFAF")))
         (*regular-file-result* nil)
         (*regular-file-results* (list (cons "/tmp/combined-games.tsv" contents)))
         (*regular-file-calls* nil)
         (games (retrodeck:load-dashboard-games "/tmp/combined-games.tsv")))
    (assert (equal (mapcar (lambda (game) (getf game :id)) games)
                   '("base-one" "upload-one")))
    (assert (string= (getf (second games) :title) "?lu?"))
    (assert (eq (getf (second games) :system) :gb))
    (assert (string= (getf (second games) :rom) "/missing/upload.gb"))
    (assert (equal *regular-file-calls*
                   '(("/tmp/combined-games.tsv" 1 65536)))))

  (assert (reject-games nil))
  (assert (reject-games ""))
  (assert (reject-games (tsv "only" "four" "fields" "here")))
  (assert (reject-games
           (game-row "Bad" "BAD ID" "nes" "/missing/bad.nes")))
  (assert (reject-games
           (game-row "bad-system" "BAD SYSTEM" "other" "/missing/bad.rom")))
  (assert (reject-games
           (game-row "bad-path" "BAD PATH" "nes" "relative.nes")))
  (assert (reject-games
           (game-row "bad-color" "BAD COLOR" "nes" "/missing/bad.nes"
                     "#12345G")))
  (assert (reject-games
           (concatenate 'string
                        (game-row "same" "ONE" "nes" "/missing/one.nes")
                        (game-row "same" "TWO" "gb" "/missing/two.gb"))))
  (assert (reject-games
           (concatenate 'string
                        (game-row "one" "ONE" "nes" "/missing/same.nes")
                        (game-row "two" "TWO" "nes" "/missing/same.nes"))))
  (assert (reject-games
           (concatenate 'string "#" (make-string 4096 :initial-element #\x)
                        (string #\Newline))))
  (let* ((prefix (concatenate 'string "#" (make-string 4095 :initial-element #\x)
                              (string #\Newline)))
         (*regular-file-result* nil)
         (*regular-file-results*
           (list (cons "/tmp/games.tsv"
                       (concatenate 'string prefix
                                    (game-row "one" "ONE" "nes"
                                              "/missing/one.nes"))))))
    (assert (= (length (retrodeck:load-dashboard-games "/tmp/games.tsv")) 1)))
  (let ((sixty-four
          (with-output-to-string (output)
            (dotimes (index 64)
              (write-string
               (game-row (format nil "g~D" index) (format nil "GAME ~D" index)
                         "nes" (format nil "/missing/g~D.nes" index))
               output)))))
    (let ((*regular-file-result* nil)
          (*regular-file-results* (list (cons "/tmp/games.tsv" sixty-four))))
      (assert (= (length (retrodeck:load-dashboard-games "/tmp/games.tsv")) 64)))
    (assert
     (reject-games
      (concatenate 'string sixty-four
                   (game-row "overflow" "OVERFLOW" "nes"
                             "/missing/overflow.nes")))))
  (let ((*regular-file-calls* nil))
    (assert-signals error (retrodeck:load-dashboard-games "relative.tsv"))
    (assert (null *regular-file-calls*)))

  (let* ((original (copy-tree retrodeck:*dashboard-palette*))
         (custom
           (loop for entry in retrodeck:*dashboard-palette*
                 for index from 1
                 collect (cons (car entry) (* index #x010101))))
         (contents
           (concatenate
            'string
            (format nil "# complete override~C~%" #\Return)
            (palette-text (reverse custom) "pixel-cog-2")))
         (*regular-file-result* nil)
         (*regular-file-results* (list (cons "/tmp/palette.tsv" contents)))
         (*regular-file-calls* nil)
         (palette (retrodeck:load-dashboard-palette "/tmp/palette.tsv")))
    (assert (equal palette custom))
    (assert (equal retrodeck:*dashboard-palette* original))
    (assert (equal *regular-file-calls* '(("/tmp/palette.tsv" 1 4096)))))
  (assert (reject-palette nil))
  (assert (reject-palette (tsv "unknown" "#000000")))
  (assert (reject-palette
           (concatenate 'string
                        (tsv "background" "#000000")
                        (tsv "background" "#111111"))))
  (assert (reject-palette
           (palette-text (butlast retrodeck:*dashboard-palette*))))
  (assert (reject-palette
           (concatenate 'string
                        (tsv "settings-icon" "pixel-cog-2")
                        (tsv "settings-icon" "pixel-cog-3")
                        (palette-text retrodeck:*dashboard-palette*))))

  (let* ((manifest (game-row "one" "ONE" "nes" "/missing/one.nes"))
         (partial (tsv "background" "#FFFFFF"))
         (original (copy-tree retrodeck:*dashboard-palette*))
         (*regular-file-result* nil)
         (*regular-file-results*
           (list (cons "/tmp/games.tsv" manifest)
                 (cons "/tmp/palette.tsv" partial)))
         (errors (make-string-output-stream)))
    (let ((*error-output* errors))
      (multiple-value-bind (games palette loaded-p)
          (retrodeck:load-dashboard-bootstrap "/tmp/games.tsv"
                                              "/tmp/palette.tsv")
        (assert (not loaded-p))
        (assert (= (length games) 8))
        (assert (equal palette original))
        (setf (cdar palette) #xffffff
              (char (getf (second games) :title) 0) #\X)
        (assert (equal retrodeck:*dashboard-palette* original))
        (assert (string= (getf (first retrodeck:*dashboard-built-in-applications*)
                               :title)
                         "LUA REPL"))))
    (assert (search "using startup dashboard palette"
                    (get-output-stream-string errors)))))

(assert (null (retrodeck:dashboard-application "missing")))
(let ((application (retrodeck:dashboard-application "terminal")))
  (setf (char (getf application :title) 0) #\X)
  (assert (string= (getf (retrodeck:dashboard-application "terminal") :title)
                   "TERMINAL")))

(let ((plan
        (retrodeck:dashboard-launch-plan
         '(:id "mario" :title "SUPER MARIO BROS." :system :nes
           :rom "/mnt/data/roms/nes/super-mario-bros.nes" :color #xd78787)
         42 :wayland t
         :volume-state "/mnt/data/nes-deck/state/menu-volume.state")))
  (assert (equal plan
                 '(:executable "/mnt/data/nes-deck/nes-deck"
                   :arguments ("/mnt/data/roms/nes/super-mario-bros.nes")
                   :environment
                   (("RETRO_DECK_VOLUME_PERCENT" . "42")
                    ("RETRO_DECK_EXIT_HINT" . "1")
                    ("RETRO_DECK_PRESENTATION" . "layer-shell")
                    ("RETRO_DECK_VOLUME_STATE" .
                     "/mnt/data/nes-deck/state/menu-volume.state"))
                   :label "mario"
                   :touch-supervision t
                   :mirror-console nil))))

(let ((plan
        (retrodeck:dashboard-launch-plan
         '(:id "zelda-oracle" :title "ZELDA ORACLE" :system :gbc
           :rom "/mnt/data/roms/gbc/zelda-oracle.gbc" :color #x87d787)
         55)))
  (assert (equal plan
                 '(:executable "/mnt/data/nes-deck/gb-deck"
                   :arguments ("/mnt/data/roms/gbc/zelda-oracle.gbc")
                   :environment
                   (("RETRO_DECK_VOLUME_PERCENT" . "55")
                    ("RETRO_DECK_EXIT_HINT" . "1"))
                   :label "zelda-oracle"
                   :touch-supervision t
                   :mirror-console nil))))

(let ((plan
        (retrodeck:dashboard-launch-plan
         '(:id "ten-seconds" :title "10 SECONDS" :system :deck
           :rom "/mnt/data/nes-deck/games/ten-seconds" :color #xffaf87)
         17 :wayland t)))
  (assert (equal plan
                 '(:executable "/mnt/data/nes-deck/ten-seconds-deck"
                   :arguments nil
                   :environment
                   (("RETRO_DECK_VOLUME_PERCENT" . "17")
                    ("RETRO_DECK_PRESENTATION" . "layer-shell"))
                   :label "ten-seconds"
                   :touch-supervision t
                   :mirror-console nil))))

(let ((plan
        (retrodeck:dashboard-launch-plan
         (retrodeck:dashboard-application "chiptunes") 63)))
  (assert (equal plan
                 '(:executable "/mnt/data/nes-deck/chiptune-deck"
                   :arguments ("/mnt/data/chiptunes")
                   :environment (("RETRO_DECK_VOLUME_PERCENT" . "63"))
                   :label "chiptunes"
                   :touch-supervision nil
                   :mirror-console nil))))

(let ((plan
        (retrodeck:dashboard-launch-plan
         (retrodeck:dashboard-application "lisp-repl") 42 :keymap "cz")))
  (assert (equal plan
                 '(:executable
                   "/mnt/data/nes-deck/terminal/retro-terminal"
                   :arguments ("lisp")
                   :environment (("RETRO_DECK_KEYMAP" . "cz"))
                   :label "lisp REPL"
                   :touch-supervision t
                   :mirror-console t))))

(let* ((plan (retrodeck:dashboard-launch-plan
              (retrodeck:dashboard-application "terminal") 42 :keymap "cz"))
       (fixtures '(((0 0 -1 -1 "exec failed" 0)
                    "TERMINAL ERROR - CHECK LOG")
                   ((0 0 -1 -1 nil 0) "TERMINAL DID NOT START")
                   ((1 1 -1 15 nil 0) "RETURNED FROM TERMINAL")
                   ((1 0 0 -1 nil 0) "TERMINAL EXITED")
                   ((1 0 7 -1 nil 0) "TERMINAL EXITED (STATUS 7)")
                   ((1 0 -1 15 nil 0) "TERMINAL STOPPED (SIGNAL 15)")
                   ((1 0 -1 -1 nil 0) "TERMINAL STOPPED"))))
  (assert (string= (retrodeck:dashboard-terminal-title plan) "TERMINAL"))
  (assert (string= (retrodeck:dashboard-terminal-starting-status plan)
                   "STARTING TERMINAL"))
  (dolist (fixture fixtures)
    (destructuring-bind (native-result expected) fixture
      (assert
       (string=
        (retrodeck:dashboard-terminal-result-status
         plan (retrodeck::decode-native-child-result native-result))
        expected))))
  (let ((repl-plan (retrodeck:dashboard-launch-plan
                    (retrodeck:dashboard-application "lisp-repl")
                    42 :keymap "cz")))
    (assert (string= (retrodeck:dashboard-terminal-title repl-plan)
                     "LISP REPL"))
    (assert (string= (retrodeck:dashboard-terminal-starting-status repl-plan)
                     "STARTING LISP REPL"))))

(dolist (result '((2 0 -1 -1 nil 0)
                  (1 -1 -1 -1 nil 0)
                  (1 0 -2 -1 nil 0)
                  (1 0 -1 0 nil 0)
                  (0 1 -1 -1 nil 0)
                  (0 0 0 -1 nil 0)
                  (1 0 0 15 nil 0)
                  (1 0 0 -1 nil 2)
                  (0 0 -1 -1 nil 1)
                  (1 1 -1 15 nil 1)
                  (1 0 0 -1 7 0)))
  (assert-signals error (retrodeck::decode-native-child-result result)))

(let* ((plan (retrodeck:dashboard-launch-plan
              (retrodeck:dashboard-application "terminal") 42 :keymap "cz"))
       (before *finish-count*))
  (setf *terminal-result* '(1 0 0 -1 nil 0)
        *terminal-arguments* nil
        retrodeck::*menu-sound-input-until-ms* 100)
  (assert (equal (retrodeck:run-dashboard-terminal plan)
                 '(:started t :exited-for-touch nil
                   :exit-code 0 :signal nil :error nil
                   :shutdown-requested nil)))
  (assert (= *finish-count* (1+ before)))
  (assert (= retrodeck::*menu-sound-input-until-ms* 0))
  (assert (equal *terminal-arguments*
                 '("/mnt/data/nes-deck/terminal/retro-terminal"
                   "cz" "shell" "terminal")))
  (let ((before-no-finish *finish-count*))
    (retrodeck:run-dashboard-terminal plan nil)
    (assert (= *finish-count* before-no-finish))))

(let* ((plan '(:executable "/tmp/retrodeck-game"
               :arguments ("first argument" "second")
               :environment (("RETRODECK_ALPHA" . "alpha value")
                             ("RETRODECK_BETA" . "beta"))
               :label "alpha"
               :touch-supervision t
               :mirror-console nil)))
  (setf *child-result* '(1 0 7 -1 nil 0)
        *child-arguments* nil)
  (assert (equal (retrodeck::run-dashboard-child plan)
                 '(:started t :exited-for-touch nil
                   :exit-code 7 :signal nil :error nil
                   :shutdown-requested nil)))
  (assert (equal *child-arguments*
                 '("/tmp/retrodeck-game"
                   ("first argument" "second")
                   (("RETRODECK_ALPHA" . "alpha value")
                    ("RETRODECK_BETA" . "beta"))
                   "alpha" 1)))
  (setf (getf plan :touch-supervision) nil
        *child-result* '(1 0 -1 15 nil 1))
  (assert (equal (retrodeck:run-dashboard-launch plan :game)
                 '(:child-returned :shutdown t :touch-disconnected t
                   :result (:started t :exited-for-touch nil
                            :exit-code nil :signal 15 :error nil
                            :shutdown-requested t))))
  (assert (zerop (fifth *child-arguments*))))

(let ((plan
        (retrodeck:dashboard-launch-plan
         (retrodeck:dashboard-application "reboot") 42)))
  (assert (equal plan
                 '(:executable "/sbin/reboot"
                   :arguments nil
                   :environment nil
                   :label "reboot"
                   :touch-supervision t
                   :mirror-console nil))))

(let* ((application '(:id "alpha" :title "ALPHA" :system :nes
                      :rom "/tmp/alpha.nes" :color #x5f87ff))
       (plan (retrodeck:dashboard-launch-plan application 42 :wayland t
                                               :volume-state "/tmp/volume.state"))
       (state (retrodeck:dashboard-loop-initial-state (list application)))
       (runtime (retrodeck:make-dashboard-runtime :wayland t
                                                   :clock (lambda () 7000))))
  (setf (getf state :active-launch)
        (list :kind :game :application application :plan plan)
        *child-result* '(1 0 0 -1 nil 0)
        *child-arguments* nil)
  (assert-runtime-effect
   runtime (list :launch plan) state
   '(:child-returned :shutdown nil :touch-disconnected nil
     :result (:started t :exited-for-touch nil
              :exit-code 0 :signal nil :error nil
              :shutdown-requested nil)))
  (assert (equal *child-arguments*
                 '("/mnt/data/nes-deck/nes-deck"
                   ("/tmp/alpha.nes")
                   (("RETRO_DECK_VOLUME_PERCENT" . "42")
                    ("RETRO_DECK_EXIT_HINT" . "1")
                    ("RETRO_DECK_PRESENTATION" . "layer-shell")
                    ("RETRO_DECK_VOLUME_STATE" . "/tmp/volume.state"))
                   "alpha" 1)))
  (setf *child-result* '(1 0 -1 15 nil 1))
  (let ((completion
          (runtime-effect runtime (list :launch plan) state)))
    (multiple-value-bind (stopped effects)
        (retrodeck:dashboard-reduce state completion)
      (assert (null (getf stopped :active-launch)))
      (assert (equal effects '((:stop-loop)))))))

(let* ((application (retrodeck:dashboard-application "reboot"))
       (plan (retrodeck:dashboard-launch-plan application 42))
       (state (retrodeck:dashboard-loop-initial-state (list application)))
       (runtime (retrodeck:make-dashboard-runtime :wayland t)))
  (setf (getf state :active-launch)
        (list :kind :reboot :application application :plan plan)
        *child-result* '(1 0 7 -1 nil 0)
        *child-arguments* nil)
  (assert-runtime-effect
   runtime (list :launch plan) state
   '(:child-returned :shutdown nil :touch-disconnected nil
     :result (:started t :exited-for-touch nil
              :exit-code 7 :signal nil :error nil
              :shutdown-requested nil)))
  (assert (equal *child-arguments*
                 '("/sbin/reboot" nil nil "reboot" 1))))

(let* ((application (retrodeck:dashboard-application "terminal"))
       (plan (retrodeck:dashboard-launch-plan application 42 :keymap "cz"))
       (state (retrodeck:dashboard-loop-initial-state nil))
       (runtime (retrodeck:make-dashboard-runtime :wayland t))
       (before *finish-count*))
  (setf (getf state :active-launch)
        (list :kind :terminal :application application :plan plan)
        *terminal-result* '(1 0 0 -1 nil 0)
        *terminal-arguments* nil)
  (assert-runtime-effect
   runtime (list :launch plan) state
   '(:child-returned :shutdown nil :touch-disconnected nil
     :result (:started t :exited-for-touch nil
              :exit-code 0 :signal nil :error nil
              :shutdown-requested nil)))
  (assert (= *finish-count* before))
  (assert (equal *terminal-arguments*
                 '("/mnt/data/nes-deck/terminal/retro-terminal"
                   "cz" "shell" "terminal"))))

(let* ((application '(:id "ten-seconds" :title "10 SECONDS" :system :deck
                      :rom "/mnt/data/nes-deck/games/ten-seconds"
                      :color #xffaf87))
       (plan (retrodeck:dashboard-launch-plan application 42))
       (state (retrodeck:dashboard-loop-initial-state
               (list application) :now 1000 :touch-connected-p t))
       (runtime (retrodeck:make-dashboard-runtime :adopt-presentation t)))
  (setf (getf state :active-launch)
        (list :kind :game :application application :plan plan)
        (getf runtime :presentation-owned-p) t
        *child-result* '(1 0 0 -1 nil 0))
  (let ((completion
          (runtime-effect runtime (list :launch plan) state)))
    (assert (getf (rest completion) :touch-disconnected))
    (multiple-value-bind (returned return-effects)
        (retrodeck:dashboard-reduce state completion)
      (assert (not (getf returned :touch-connected-p)))
      (assert (equal return-effects '((:scan-controls :force t))))
      (multiple-value-bind (scanned scan-effects)
          (retrodeck:dashboard-reduce
           returned '(:controls-rescanned :now 5000))
        (assert (equal scan-effects '((:open-presentation))))
        (multiple-value-bind (presented presentation-effects)
            (retrodeck:dashboard-reduce scanned '(:presentation-opened))
          (assert (equal presentation-effects '((:reload-volume))))
          (multiple-value-bind (complete complete-effects)
              (retrodeck:dashboard-reduce
               presented '(:child-complete :volume 42))
            (assert (equal complete-effects '((:render) (:present))))
            (multiple-value-bind (waiting reconnect-trace)
                (retrodeck:dashboard-loop-begin-iteration
                 complete '(:now 5001 :wayland nil)
                 (lambda (effect current)
                   (declare (ignore effect current))))
              (assert (not (getf waiting :touch-connected-p)))
              (assert (equal reconnect-trace
                             '((:reap-sound) (:reconnect-touch)))))))))))

(let* ((application '(:id "alpha" :title "ALPHA" :system :nes
                      :rom "/tmp/alpha.nes" :color #x5f87ff))
       (plan (retrodeck:dashboard-launch-plan application 42 :wayland t))
       (state (retrodeck:dashboard-loop-initial-state (list application)))
       (calls 0)
       (runtime
         (retrodeck:make-dashboard-runtime
          :wayland t
          :external-effect-handler
          (lambda (effect current)
            (declare (ignore effect current))
            (incf calls)
            '(:child-returned :result (:started nil :error "external"))))))
  (setf (getf state :active-launch)
        (list :kind :game :application application :plan plan)
        *child-arguments* nil
        *terminal-arguments* nil)
  (assert-runtime-effect runtime (list :launch plan) state
                         '(:child-returned
                           :result (:started nil :error "external")))
  (assert (= calls 1))
  (assert (null *child-arguments*))
  (assert (null *terminal-arguments*)))

(assert (retrodeck:reboot-confirmation-active-p 5000 4999))
(assert (not (retrodeck:reboot-confirmation-active-p 5000 5000)))
(assert (not (retrodeck:reboot-confirmation-active-p 0 0)))

(let* ((games '((:id "alpha" :title "ALPHA" :system :nes :color #x5f87ff)
                (:id "beta" :title "BETA" :system :nes :color #xafd75f)
                (:id "gamma" :title "GAMMA" :system :nes :color #xffffaf)
                (:id "delta" :title "DELTA" :system :nes :color #xd75f5f)
                (:id "gb" :title "GB" :system :gb :color #x87af87)))
       (layout (retrodeck:render-dashboard games :nes 0 "STALE"))
       (state (retrodeck:dashboard-initial-state games)))
  (assert (equal state
                 '(:active-system :nes :game-position 0
                   :pressed-target nil :status "")))
  (assert (equal (retrodeck:dashboard-initial-state
                  '((:id "other" :system :other)))
                 '(:active-system :other :game-position 0
                   :pressed-target nil :status "")))
  (assert (equal (retrodeck:dashboard-initial-state nil)
                 '(:active-system nil :game-position 0
                   :pressed-target nil :status "")))
  (assert (eq (retrodeck:dashboard-target-at layout 12 412) :credits))
  (assert (eq (retrodeck:dashboard-target-at layout 1212 412) :settings))
  (assert (eq (retrodeck:dashboard-target-at layout 157 233) :previous))
  (assert (eq (retrodeck:dashboard-target-at layout 1045 233) :next))
  (assert (equal (retrodeck:dashboard-target-at layout 56 76)
                 '(:system :nes)))
  (assert (equal (retrodeck:dashboard-target-at layout 934 102)
                 '(:system :gb)))
  (assert (equal (retrodeck:dashboard-target-at layout 388 286)
                 '(:game 0)))
  (assert (null (retrodeck:dashboard-target-at layout 636 100)))
  (assert (null (retrodeck:dashboard-target-at layout 68 412)))

  (setf (getf state :status) "STALE")
  (assert-touch-release retrodeck:dashboard-touch-transition state layout
                        '(1084 282 t t nil) '(1084 282 nil nil t)
    (assert (eq (getf pressed :pressed-target) :next))
    (assert (null (getf state :pressed-target)))
    (assert (= (getf released :game-position) 1))
    (assert (string= (getf released :status) ""))
    (assert (null (getf released :pressed-target)))
    (assert (equal release-effect '(:render t :cue :next))))

  (assert-touch-release retrodeck:dashboard-touch-transition state layout
                        '(196 282 t t nil) '(196 282 nil nil t)
    (assert (= (getf released :game-position) 3))
    (assert (equal release-effect '(:render t :cue :previous))))

  (let ((positioned (copy-list state)))
    (setf (getf positioned :game-position) 3)
    (assert-touch-release retrodeck:dashboard-touch-transition positioned layout
                          '(346 102 t t nil) '(346 102 nil nil t)
      (assert (eq (getf released :active-system) :nes))
      (assert (zerop (getf released :game-position)))
      (assert (equal release-effect '(:render t)))))

  (assert-touch-release retrodeck:dashboard-touch-transition state layout
                        '(934 102 t t nil) '(934 102 nil nil t)
    (assert (eq (getf released :active-system) :gb))
    (assert (zerop (getf released :game-position)))
    (assert (equal release-effect '(:render t :cue :next))))

  (assert-touch-release retrodeck:dashboard-touch-transition state layout
                        '(1084 282 t t nil) '(196 282 nil nil t)
    (assert (zerop (getf released :game-position)))
    (assert (null (getf released :pressed-target)))
    (assert (null release-effect)))

  (assert-touch-release retrodeck:dashboard-touch-transition state layout
                        '(1084 282 t t nil) '(-1 -1 nil nil t)
    (assert (zerop (getf released :game-position)))
    (assert (null (getf released :pressed-target)))
    (assert (null release-effect))))

(let* ((games '((:id "alpha" :title "ALPHA" :system :nes :color #x5f87ff)
                (:id "beta" :title "BETA" :system :nes :color #xafd75f)
                (:id "gamma" :title "GAMMA" :system :nes :color #xffffaf)
                (:id "delta" :title "DELTA" :system :nes :color #xd75f5f)))
       (layout (retrodeck:render-dashboard games :nes 0 ""))
       (state (retrodeck:dashboard-initial-state games))
       (*record-interaction* t)
       (*interaction-trace* nil)
       (*play-status* 1)
       (*active-status* 0)
       (presenter (lambda ()
                    (push :present *interaction-trace*)
                    t)))
  (multiple-value-bind (pressed pressed-layout effect)
      (retrodeck:apply-dashboard-touch games state layout
                                       '(1084 282 t t nil) 42 presenter)
    (assert (eq pressed-layout layout))
    (assert (null effect))
    (assert (null *interaction-trace*))
    (multiple-value-bind (released released-layout release-effect)
        (retrodeck:apply-dashboard-touch games pressed pressed-layout
                                         '(1084 282 nil nil t) 42 presenter)
      (assert (= (getf released :game-position) 1))
      (assert (= (getf released-layout :shown-game-index) 1))
      (assert (equal release-effect '(:render t :cue :next)))
      (assert (equal (reverse *interaction-trace*)
                     '(:render :present :sound)))
      (assert (equal *play-arguments* '(659 35 0 0 42)))

      (setf *interaction-trace* nil
            *active-status* 1
            *play-status* 2)
      (multiple-value-bind (pressed-again ignored-layout ignored-effect)
          (retrodeck:apply-dashboard-touch games released released-layout
                                           '(1084 282 t t nil) 42 presenter)
        (declare (ignore ignored-layout))
        (assert (null ignored-effect))
        (multiple-value-bind (released-again final-layout final-effect)
            (retrodeck:apply-dashboard-touch games pressed-again released-layout
                                             '(1084 282 nil nil t) 42 presenter)
          (assert (= (getf released-again :game-position) 2))
          (assert (= (getf final-layout :shown-game-index) 2))
          (assert (equal final-effect '(:render t :cue :next)))
          (assert (equal (reverse *interaction-trace*)
                         '(:render :present :sound)))))))
  (setf *active-status* 0
        *play-status* 1
        retrodeck::*menu-sound-input-until-ms* 0))

(let* ((games '((:id "alpha" :title "ALPHA" :system :nes :color #x5f87ff)
                (:id "beta" :title "BETA" :system :nes :color #xafd75f)
                (:id "long-title" :title "A VERY LONG FIXTURE GAME TITLE"
                 :system :nes :color #xffd700)
                (:id "delta" :title "DELTA" :system :nes :color #xd75f5f)
                (:id "gb" :title "GB FIXTURE" :system :gb :color #x87af87)
                (:id "gbc" :title "GBC FIXTURE" :system :gbc :color #xecb6e7)
                (:id "zx" :title "ZX FIXTURE" :system :zx :color #x87afff)
                (:id "deck-fixture" :title "DECK FIXTURE" :system :deck
                 :color #xff8700)))
       (state (retrodeck:dashboard-initial-state games))
       (layout (retrodeck:render-dashboard games :nes 0 ""))
       (trace nil))
  (labels ((trace-touch (pressed-x pressed-y released-x released-y)
             (multiple-value-bind (pressed ignored)
                 (retrodeck:dashboard-touch-transition
                  state layout (list pressed-x pressed-y t t nil))
               (declare (ignore ignored))
               (multiple-value-bind (released effect)
                   (retrodeck:dashboard-touch-transition
                    pressed layout (list released-x released-y nil nil t))
                 (setf state released)
                 (when (getf effect :render)
                   (setf layout
                         (retrodeck:render-dashboard
                          games (getf state :active-system)
                          (getf state :game-position) (getf state :status))))
                 (push (list (getf state :active-system)
                             (getf state :game-position)
                             (not (null (getf effect :render)))
                             (getf effect :cue))
                       trace)))))
    (trace-touch 1084 282 1084 282)
    (trace-touch 1084 282 196 282)
    (trace-touch 1084 282 1084 282)
    (trace-touch 346 102 346 102)
    (trace-touch 346 102 346 102))
  ;; Shared with the C++ reference trace and its per-frame RGB565 hashes.
  (assert (equal (nreverse trace)
                 '((:nes 1 t :next)
                   (:nes 1 nil nil)
                   (:nes 2 t :next)
                   (:gb 0 t :next)
                   (:gb 0 t nil)))))

(let* ((games '((:id "alpha" :title "ALPHA" :system :nes
                 :color #x5f87ff)
                (:id "beta" :title "BETA" :system :nes
                 :color #xafd75f)
                (:id "gb" :title "GB" :system :gb :color #x87af87)))
       (layout (retrodeck:render-dashboard games :nes 0 ""))
       (state (retrodeck:dashboard-loop-initial-state
               games :network '(:wifi "TEST") :now 25)))
  (assert (eq (getf state :view) :dashboard))
  (assert (equal (getf state :network) '(:wifi "TEST")))
  (assert (= (getf state :network-refreshed-at) 25))
  (assert (eq (getf (getf state :settings) :selected) :volume-down))
  (assert (not (getf (getf state :settings) :open)))

  (let* ((armed (copy-list state))
         (dashboard (copy-list (getf state :dashboard))))
    (setf (getf dashboard :pressed-target) :settings
          (getf armed :dashboard) dashboard)
    (multiple-value-bind (next effects)
        (retrodeck:dashboard-reduce
         armed (list :controls :gamepad-actions '(:right)
                     :keyboard-actions nil :layout layout :now 100
                      :controller-quarantined-p nil))
      (assert (= (getf (getf next :dashboard) :game-position) 1))
      (assert (null (getf (getf next :dashboard) :pressed-target)))
      (assert (eq (getf (getf armed :dashboard) :pressed-target) :settings))
      (assert (equal effects
                     '((:discard-touch) (:render) (:present) (:cue :next))))))

  (multiple-value-bind (next effects)
      (retrodeck:dashboard-reduce
       state (list :controls :gamepad-actions '(:confirm)
                   :keyboard-actions '(:right) :layout layout :now 150
                   :controller-quarantined-p t))
    (assert (= (getf (getf next :dashboard) :game-position) 1))
    (assert (equal (getf (getf next :controller-guard) :edge-times) '(150)))
    (assert (equal effects
                   '((:discard-touch) (:render) (:present) (:cue :next)))))

  (multiple-value-bind (next effects)
      (retrodeck:dashboard-reduce
       state (list :controls :gamepad-actions '(:confirm)
                   :keyboard-actions nil :layout layout :now 175
                   :controller-quarantined-p t))
    (assert (zerop (getf (getf next :dashboard) :game-position)))
    (assert (equal (getf (getf next :controller-guard) :edge-times) '(175)))
    (assert (null effects)))

  (assert-signals error
                  (retrodeck:dashboard-reduce
                   state (list :controls :gamepad-actions nil :keyboard-actions nil
                               :layout layout :now 190)))

  (let ((suspended (copy-list state)))
    (setf (getf suspended :controller-guard)
          '(:edge-times (0) :suspended t :last-edge-at 100)
          (getf suspended :last-control-scan-ms) 1000)
    (multiple-value-bind (waiting effects)
        (retrodeck:dashboard-reduce
         suspended '(:begin-iteration :now 1099 :wayland t))
      (assert (getf (getf waiting :controller-guard) :suspended))
      (assert (equal effects '((:reap-sound)))))
    (multiple-value-bind (recovered effects)
        (retrodeck:dashboard-reduce
         suspended '(:begin-iteration :now 1100 :wayland t))
      (assert (equal (getf recovered :controller-guard)
                     '(:edge-times nil :suspended nil :last-edge-at nil)))
      (assert (equal effects
                     '((:reap-sound) (:controller-resumed))))))

  (multiple-value-bind (next effects)
      (retrodeck:dashboard-reduce
       state (list :controls :gamepad-actions '(:system-previous)
                   :keyboard-actions nil :layout layout :now 200
                    :controller-quarantined-p nil))
    (assert (eq (getf (getf next :dashboard) :active-system) :gb))
    (assert (equal effects
                   '((:discard-touch) (:render) (:present)
                     (:cue :previous)))))

  (multiple-value-bind (settings effects)
      (retrodeck:dashboard-reduce
       state (list :controls :gamepad-actions '(:settings)
                   :keyboard-actions nil :layout layout :now 250
                    :controller-quarantined-p nil))
    (assert (eq (getf settings :view) :settings))
    (assert (getf (getf settings :settings) :open))
    (assert (eq (getf (getf settings :settings) :selected) :volume-down))
    (assert (equal effects
                   '((:discard-touch) (:render) (:present)
                     (:cue :confirm))))
    (multiple-value-bind (moved move-effects)
        (retrodeck:dashboard-reduce
         settings (list :controls :gamepad-actions nil
                        :keyboard-actions '(:right) :layout layout :now 275
                        :controller-quarantined-p nil))
      (assert (eq (getf (getf moved :settings) :selected) :volume-up))
      (assert (equal move-effects
                     '((:discard-touch) (:render) (:present) (:cue :next))))
      (multiple-value-bind (closed close-effects)
          (retrodeck:dashboard-reduce
           moved (list :controls :gamepad-actions nil
                       :keyboard-actions '(:back) :layout layout :now 300
                       :controller-quarantined-p nil))
        (assert (eq (getf closed :view) :dashboard))
        (assert (equal close-effects
                       '((:discard-touch) (:render) (:present)
                         (:cue :back)))))))

  (multiple-value-bind (pressed effects)
      (retrodeck:dashboard-reduce
       state (list :touch :report '(1084 282 t t nil) :layout layout
                   :now 350))
    (assert (null effects))
    (multiple-value-bind (released release-effects)
        (retrodeck:dashboard-reduce
         pressed (list :touch :report '(1084 282 nil nil t) :layout layout
                       :now 351))
      (assert (= (getf (getf released :dashboard) :game-position) 1))
      (assert (equal release-effects
                     '((:render) (:present) (:cue :next))))))

  (let* ((modal (copy-list state))
         (credits (copy-list (getf state :credits))))
    (setf (getf modal :view) :credits
          (getf credits :pressed-target) :close
          (getf modal :credits) credits)
    (multiple-value-bind (next effects)
        (retrodeck:dashboard-reduce
         modal (list :controls :gamepad-actions '(:right)
                     :keyboard-actions nil :layout layout :now 400
                      :controller-quarantined-p nil))
      (assert (eq (getf (getf next :credits) :pressed-target) :close))
      (assert (null effects)))))

(labels ((touch-pair (state layout x y now)
           (multiple-value-bind (pressed press-effects)
               (retrodeck:dashboard-reduce
                state (list :touch :report (list x y t t nil)
                            :layout layout :now now))
             (assert (null press-effects))
             (retrodeck:dashboard-reduce
              pressed (list :touch :report (list x y nil nil t)
                            :layout layout :now (1+ now))))))
  (let* ((games (runtime-test-games))
         (dashboard-layout (retrodeck:render-dashboard games :nes 0 ""))
         (state (retrodeck:dashboard-loop-initial-state
                 games :volume 42 :brightness 60 :keymap "us")))
    (multiple-value-bind (settings open-effects)
        (touch-pair state dashboard-layout 1220 420 1000)
      (assert (eq (getf settings :view) :settings))
      (assert (eq (getf (getf settings :settings) :selected) :volume-down))
      (assert (equal open-effects
                     '((:render) (:present) (:cue :confirm))))
      (let ((settings-layout
              (retrodeck:render-dashboard-settings 42 60 "us" :volume-down
                                                   "" nil)))
        (multiple-value-bind (requested request-effects)
            (retrodeck:dashboard-reduce
             settings
             (list :controls :gamepad-actions '(:confirm)
                   :keyboard-actions nil :layout settings-layout :now 1010
                   :controller-quarantined-p nil))
          (let ((plan (getf requested :pending-settings-plan)))
            (assert (eq (getf plan :action) :volume))
            (assert (= (getf plan :value) 37))
            (assert (equal request-effects
                           (list '(:discard-touch)
                                 (list :settings-action plan))))
            (assert-signals error
                            (retrodeck:dashboard-reduce
                             requested
                             (list :touch :report '(0 0 t t nil)
                                   :layout settings-layout :now 1011)))
            (multiple-value-bind (completed complete-effects)
                (retrodeck:dashboard-reduce
                 requested '(:settings-result :succeeded-p t))
              (assert (= (getf (getf completed :settings) :volume) 37))
              (assert (string= (getf (getf completed :settings) :status)
                               "GAME VOLUME 37%"))
              (assert (equal complete-effects
                             '((:render) (:present)
                               (:cue :volume :report-result t))))
              (multiple-value-bind (tone-failed tone-effects)
                  (retrodeck:dashboard-reduce
                   completed '(:volume-tone-result :succeeded-p nil))
                (assert
                 (string= (getf (getf tone-failed :settings) :status)
                          "VOLUME SAVED; CONFIRMATION TONE FAILED"))
                (assert (equal tone-effects '((:render) (:present))))))))

        (multiple-value-bind (wifi open-wifi-effects)
            (touch-pair settings settings-layout 1000 50 1020)
          (assert (eq (getf wifi :view) :wifi))
          (assert (getf (getf wifi :settings) :open))
          (assert (getf (getf wifi :wifi) :open))
          (assert (equal open-wifi-effects
                         '((:render) (:present) (:cue :confirm))))
          (let ((wifi-layout
                  (retrodeck:render-dashboard-wifi (getf wifi :wifi) nil)))
            (multiple-value-bind (focused focus-effects)
                (touch-pair wifi wifi-layout 340 20 1030)
              (assert (eq (getf (getf focused :wifi) :field) :ssid))
              (assert (equal focus-effects
                             '((:render) (:cue :next) (:present)))))
            (multiple-value-bind (unchanged blank-effects)
                (touch-pair wifi wifi-layout -1 -1 1035)
              (assert (eq (getf unchanged :view) :wifi))
              (assert (equal blank-effects '((:present)))))
            (multiple-value-bind (closed close-effects)
                (touch-pair wifi wifi-layout 20 20 1040)
              (assert (eq (getf closed :view) :settings))
              (assert (string= (getf (getf closed :settings) :status)
                               "WIFI EDITOR CLOSED"))
              (assert (equal close-effects
                             '((:render) (:cue :back) (:present))))))))))

  (let* ((games '((:id "alpha" :title "ALPHA" :system :nes
                   :color #x5f87ff)))
         (dashboard-layout (retrodeck:render-dashboard games :nes 0 ""))
         (state (retrodeck:dashboard-loop-initial-state
                 games :wifi-state
                 (retrodeck:wifi-initial-state
                  :ssid "DEMO" :passphrase "password"))))
    (multiple-value-bind (settings ignored)
        (touch-pair state dashboard-layout 1220 420 1100)
      (declare (ignore ignored))
      (let ((settings-layout
              (retrodeck:render-dashboard-settings
               42 100 "us" :volume-down "" nil)))
        (multiple-value-bind (wifi ignored)
            (touch-pair settings settings-layout 1000 50 1110)
          (declare (ignore ignored))
          (let ((wifi-layout
                  (retrodeck:render-dashboard-wifi (getf wifi :wifi) nil)))
            (multiple-value-bind (saving save-effects)
                (touch-pair wifi wifi-layout 1000 20 1120)
              (let ((plan (getf saving :pending-wifi-plan)))
                (assert (eq (getf plan :action) :save))
                (assert (equal save-effects
                               (list (list :wifi-action plan))))
                (assert-signals error
                                (retrodeck:dashboard-reduce
                                 saving
                                 (list :touch :report '(20 20 nil nil t)
                                       :layout wifi-layout :now 1122)))
                (multiple-value-bind (saved completion-effects)
                    (retrodeck:dashboard-reduce
                     saving '(:wifi-result :succeeded-p t))
                  (assert (string= (getf (getf saved :wifi) :passphrase) ""))
                  (assert (string= (getf (getf saved :wifi) :status)
                                   "WIFI SAVED - USED AFTER CURRENT WIFI DISCONNECTS"))
                  (assert (equal completion-effects
                                 '((:render) (:cue :confirm) (:present))))))))))))

  (let* ((games '((:id "alpha" :title "ALPHA" :system :nes
                   :color #x5f87ff)))
         (layout (retrodeck:render-dashboard games :nes 0 ""))
         (state (retrodeck:dashboard-loop-initial-state games)))
    (multiple-value-bind (credits open-effects)
        (touch-pair state layout 20 420 1200)
      (assert (eq (getf credits :view) :credits))
      (assert (= (getf credits :credits-started-at) 1201))
      (assert (equal open-effects
                     '((:render) (:present) (:cue :confirm))))
      (multiple-value-bind (closed close-effects)
          (touch-pair credits '(:close (1212 12 56 56)) 1220 20 1210)
        (assert (eq (getf closed :view) :dashboard))
        (assert (equal close-effects
                       '((:render) (:present) (:cue :back)))))))

  (let* ((games (list '(:id "alpha" :title "ALPHA" :system :nes
                        :color #x5f87ff :rom "/tmp/alpha.nes")
                      (retrodeck:dashboard-application "terminal")
                      (retrodeck:dashboard-application "reboot")))
         (deck-layout (retrodeck:render-dashboard games :deck 1 ""))
         (state (retrodeck:dashboard-loop-initial-state games))
         (dashboard (copy-list (getf state :dashboard))))
    (setf (getf dashboard :active-system) :deck
          (getf dashboard :game-position) 1
          (getf state :dashboard) dashboard)
    (multiple-value-bind (armed arm-effects)
        (retrodeck:dashboard-reduce
         state (list :controls :gamepad-actions '(:confirm)
                     :keyboard-actions nil :layout deck-layout :now 1000
                     :controller-quarantined-p nil))
      (assert (= (getf armed :reboot-deadline) 5000))
      (assert (null (getf armed :pending-launch)))
      (assert (string= (getf (getf armed :dashboard) :status)
                       retrodeck:*dashboard-reboot-confirmation-text*))
      (assert (equal arm-effects
                     '((:discard-touch) (:render) (:present)
                       (:cue :confirm))))
      (multiple-value-bind (still-armed effects)
          (retrodeck:dashboard-reduce armed '(:tick :now 4999))
        (assert (= (getf still-armed :reboot-deadline) 5000))
        (assert (null effects)))
      (multiple-value-bind (expired effects)
          (retrodeck:dashboard-reduce armed '(:tick :now 5000))
        (assert (zerop (getf expired :reboot-deadline)))
        (assert (string= (getf (getf expired :dashboard) :status) ""))
        (assert (equal effects '((:render) (:present)))))
      (multiple-value-bind (confirmed confirm-effects)
          (retrodeck:dashboard-reduce
           armed (list :controls :gamepad-actions '(:confirm)
                       :keyboard-actions nil :layout deck-layout :now 4999
                       :controller-quarantined-p nil))
        (assert (equal (getf confirmed :pending-launch)
                       '(:kind :reboot :game-index 2 :touch-batch nil)))
        (assert (zerop (getf confirmed :reboot-deadline)))
        (assert (equal confirm-effects
                       '((:discard-touch) (:cue :confirm)))))
      (multiple-value-bind (pressed ignored)
          (retrodeck:dashboard-reduce
           armed (list :touch :report '(-1 -1 t t nil)
                       :layout deck-layout :now 4500))
        (declare (ignore ignored))
        (multiple-value-bind (cancelled effects)
            (retrodeck:dashboard-reduce
             pressed (list :touch :report '(-1 -1 nil nil t)
                           :layout deck-layout :now 4501))
          (assert (zerop (getf cancelled :reboot-deadline)))
          (assert (string= (getf (getf cancelled :dashboard) :status) ""))
          (assert (null effects)))))

    (let ((nes-layout (retrodeck:render-dashboard games :nes 0 "")))
      (multiple-value-bind (requested effects)
          (retrodeck:dashboard-reduce
           (retrodeck:dashboard-loop-initial-state games)
           (list :controls :gamepad-actions '(:confirm)
                 :keyboard-actions nil :layout nes-layout :now 1300
                 :controller-quarantined-p nil))
        (assert (equal (getf requested :pending-launch)
                       '(:kind :game :game-index 0 :touch-batch nil)))
        (assert (equal effects '((:discard-touch) (:cue :confirm))))))))

(let* ((games (runtime-test-games))
       (layout (retrodeck:render-dashboard games :nes 0 ""))
       (state (retrodeck:dashboard-loop-initial-state games :now 100)))
  (assert (= (retrodeck:dashboard-loop-poll-timeout state) 250))
  (assert-signals error
                  (retrodeck:dashboard-reduce state '(:prepare-launch)))
  (multiple-value-bind (touch-pressed ignored)
      (retrodeck:dashboard-reduce
       state (list :touch :report '(640 286 t t nil)
                   :layout layout :now 90))
    (declare (ignore ignored))
    (multiple-value-bind (touch-requested touch-effects)
        (retrodeck:dashboard-reduce
         touch-pressed (list :touch :report '(640 286 nil nil t)
                             :layout layout :now 91))
      (assert (getf (getf touch-requested :pending-launch) :touch-batch))
      (assert (equal touch-effects '((:cue :confirm))))
      (multiple-value-bind (continued effects)
          (retrodeck:dashboard-reduce
           touch-requested
           (list :touch :report '(-1 -1 nil nil t)
                 :layout layout :now 92))
        (assert (getf continued :pending-launch))
        (assert (null effects)))))
  (multiple-value-bind (settings ignored)
      (retrodeck:dashboard-reduce
       state (list :controls :gamepad-actions '(:settings)
                   :keyboard-actions nil :layout layout :now 101
                   :controller-quarantined-p nil))
    (declare (ignore ignored))
    (multiple-value-bind (early effects)
        (retrodeck:dashboard-reduce settings '(:tick :now 2099))
      (assert (null effects))
      (multiple-value-bind (refresh refresh-effects)
          (retrodeck:dashboard-reduce early '(:tick :now 2100))
        (assert (= (getf refresh :network-refreshed-at) 2100))
        (assert (getf refresh :pending-network))
        (assert (equal refresh-effects '((:network-action))))
        (assert-signals error
                        (retrodeck:dashboard-reduce refresh '(:tick :now 2101)))
        (multiple-value-bind (unchanged unchanged-effects)
            (retrodeck:dashboard-reduce
             refresh '(:network-result :network nil))
          (assert (null unchanged-effects))
          (multiple-value-bind (again again-effects)
              (retrodeck:dashboard-reduce unchanged '(:tick :now 4100))
            (assert (equal again-effects '((:network-action))))
            (multiple-value-bind (changed changed-effects)
                (retrodeck:dashboard-reduce
                 again '(:network-result :network (:wifi "NEW")))
              (assert (equal (getf changed :network) '(:wifi "NEW")))
              (assert (equal changed-effects '((:render) (:present))))))))))

  (multiple-value-bind (lost effects)
      (retrodeck:dashboard-reduce state '(:touch-lost))
    (assert (string= (getf (getf lost :dashboard) :status)
                     "WAITING FOR TOUCHSCREEN"))
    (assert (equal effects '((:render) (:present))))
    (multiple-value-bind (restored restored-effects)
        (retrodeck:dashboard-reduce lost '(:touch-reconnected))
      (assert (string= (getf (getf restored :dashboard) :status)
                       "TOUCHSCREEN RECONNECTED"))
      (assert (equal restored-effects '((:render) (:present))))))

  (let ((credits (copy-list state)))
    (setf (getf credits :view) :credits)
    (assert (= (retrodeck:dashboard-loop-poll-timeout credits) 40))
    (multiple-value-bind (animated effects)
        (retrodeck:dashboard-reduce credits '(:tick :now 200))
      (declare (ignore animated))
      (assert (equal effects '((:render) (:present))))))
  (let ((credits (retrodeck:dashboard-loop-initial-state
                  games :reduced-motion t)))
    (setf (getf credits :view) :credits)
    (assert (= (retrodeck:dashboard-loop-poll-timeout credits) 250))
    (multiple-value-bind (static effects)
        (retrodeck:dashboard-reduce credits '(:tick :now 200))
      (declare (ignore static))
      (assert (null effects))))

  (multiple-value-bind (requested request-effects)
      (retrodeck:dashboard-reduce
       state (list :controls :gamepad-actions '(:confirm)
                   :keyboard-actions nil :layout layout :now 500
                   :controller-quarantined-p nil))
    (assert (equal request-effects '((:discard-touch) (:cue :confirm))))
    (assert-signals error
                    (retrodeck:dashboard-reduce requested '(:tick :now 501)))
    (multiple-value-bind (launching launch-effects)
        (retrodeck:dashboard-reduce
         requested '(:prepare-launch :wayland t
                     :volume-state "/tmp/volume.state"))
      (let* ((launch (getf launching :active-launch))
             (plan (getf launch :plan)))
        (assert (eq (getf launch :kind) :game))
        (assert (string= (getf (getf launching :dashboard) :status)
                         "STARTING ALPHA"))
        (assert (equal launch-effects
                       (list '(:render) '(:present) '(:finish-sound)
                             '(:close-controls) (list :launch plan))))
        (assert (equal (cdr (assoc "RETRO_DECK_PRESENTATION"
                                   (getf plan :environment) :test #'string=))
                       "layer-shell"))
        (assert-signals error
                        (retrodeck:dashboard-reduce
                         launching
                         (list :controls :gamepad-actions nil :keyboard-actions '(:right)
                               :layout layout :now 501
                               :controller-quarantined-p nil)))
        (multiple-value-bind (returned-child recovery-effects)
            (retrodeck:dashboard-reduce
             launching
             '(:child-returned
               :result (:started t :exited-for-touch t
                        :exit-code nil :signal nil :error nil)))
          (assert (equal recovery-effects '((:scan-controls :force t))))
          (multiple-value-bind (scanned scan-effects)
              (retrodeck:dashboard-reduce
               returned-child '(:controls-rescanned :now 600))
            (assert (equal scan-effects '((:open-presentation))))
            (multiple-value-bind (opened open-effects)
                (retrodeck:dashboard-reduce scanned '(:presentation-opened))
              (assert (equal open-effects '((:reload-volume))))
              (multiple-value-bind (returned return-effects)
                  (retrodeck:dashboard-reduce
                   opened '(:child-complete :volume 55))
                (assert (null (getf returned :active-launch)))
                (assert (= (getf (getf returned :settings) :volume) 55))
                (assert (= (getf (getf returned :settings)
                                 :last-audible-volume) 55))
                (assert (string= (getf (getf returned :dashboard) :status)
                                 "RETURNED FROM ALPHA"))
                (assert (equal return-effects
                               '((:render) (:present))))))))
        (let ((recovering (copy-list launching)))
          (setf (getf recovering :pending-child-result)
                '(:started t :exited-for-touch nil
                  :exit-code 7 :signal nil :error nil)
                (getf recovering :child-return-stage) :volume)
          (multiple-value-bind (failed ignored)
              (retrodeck:dashboard-reduce recovering '(:child-complete))
            (declare (ignore ignored))
            (assert (string= (getf (getf failed :dashboard) :status)
                             "ALPHA EXITED (STATUS 7)"))))
        (let ((recovering (copy-list launching)))
          (setf (getf recovering :pending-child-result)
                '(:started t :exited-for-touch nil
                  :exit-code nil :signal 15 :error nil)
                (getf recovering :child-return-stage) :volume)
          (multiple-value-bind (stopped ignored)
              (retrodeck:dashboard-reduce recovering '(:child-complete))
            (declare (ignore ignored))
            (assert (string= (getf (getf stopped :dashboard) :status)
                             "ALPHA STOPPED (SIGNAL 15)"))))))))

(let* ((games (runtime-test-games t))
       (state (retrodeck:dashboard-loop-initial-state games))
       (layout (retrodeck:render-dashboard games :nes 0 ""))
       (now 3000))
  (labels ((current-layout () layout)
           (handle-effect (effect current)
             (when (eq (first effect) :render)
               (setf layout (retrodeck:render-dashboard-loop-state current now)))
             nil))
    (multiple-value-bind (next trace)
        (retrodeck:dashboard-loop-dispatch-input
         state
         '(:gamepad-actions (:right) :keyboard-actions nil
           :touch-reports ((1220 420 t t nil) (1220 420 nil nil t))
           :now 3000 :controller-quarantined-p nil)
         #'current-layout #'handle-effect)
      (assert (= (getf (getf next :dashboard) :game-position) 1))
      (assert (eq (getf next :view) :dashboard))
      (assert (equal trace
                     '((:discard-touch) (:render) (:present) (:cue :next))))))

  (setf state (retrodeck:dashboard-loop-initial-state games)
        layout (retrodeck:render-dashboard games :nes 0 ""))
  (labels ((current-layout () layout)
           (handle-effect (effect current)
             (when (eq (first effect) :render)
               (setf layout (retrodeck:render-dashboard-loop-state current now)))
             nil))
    (multiple-value-bind (next trace)
        (retrodeck:dashboard-loop-dispatch-input
         state
         '(:gamepad-actions nil :keyboard-actions nil
           :touch-reports ((1220 420 t t nil) (1220 420 nil nil t)
                           (1000 50 t t nil) (1000 50 nil nil t))
           :touch-times (3010 3011 3012 3013)
           :now 3010 :controller-quarantined-p nil)
         #'current-layout #'handle-effect)
      (assert (eq (getf next :view) :wifi))
      (assert (equal trace
                     '((:render) (:present) (:cue :confirm)
                       (:render) (:present) (:cue :confirm))))))

  (setf state (retrodeck:dashboard-loop-initial-state games)
        layout (retrodeck:render-dashboard games :nes 0 ""))
  (labels ((current-layout () layout)
           (handle-effect (effect current)
             (case (first effect)
               (:render
                (setf layout
                      (retrodeck:render-dashboard-loop-state current now))
                nil)
               (:launch
                '(:child-returned
                  :result (:started t :exited-for-touch nil
                           :exit-code 0 :signal nil :error nil)))
               (:scan-controls '(:controls-rescanned :now 3020))
               (:open-presentation '(:presentation-opened))
               (:reload-volume '(:child-complete :volume 47))
               (otherwise nil))))
    (multiple-value-bind (next trace)
        (retrodeck:dashboard-loop-dispatch-input
         state
         '(:gamepad-actions (:confirm) :keyboard-actions nil
           :touch-reports ((1220 420 t t nil) (1220 420 nil nil t))
           :now 3020 :controller-quarantined-p nil :wayland t
           :volume-state "/tmp/volume.state")
         #'current-layout #'handle-effect)
      (assert (eq (getf next :view) :dashboard))
      (assert (null (getf next :active-launch)))
      (assert (= (getf (getf next :settings) :volume) 47))
      (assert (= (getf next :last-control-scan-ms) 3020))
      (assert (string= (getf (getf next :dashboard) :status)
                       "ALPHA EXITED"))
      (assert (equal (mapcar #'first trace)
                     '(:discard-touch :cue :render :present :finish-sound
                       :close-controls :launch :scan-controls
                       :open-presentation :reload-volume :render :present))))))

(let* ((games (runtime-test-games t))
       (state (retrodeck:dashboard-loop-initial-state
               games :now 1000 :touch-connected-p nil))
       (layout (retrodeck:render-dashboard games :nes 0 ""))
       (now 1100))
  (setf (getf state :controller-guard)
        '(:edge-times (0) :suspended t :last-edge-at 100)
        (getf state :last-control-scan-ms) 0
        (getf state :last-touch-reconnect-ms) 100)
  (labels ((current-layout () layout)
           (handle-effect (effect current)
             (case (first effect)
               (:render
                (setf layout
                      (retrodeck:render-dashboard-loop-state current now))
                nil)
               (:reconnect-touch '(:touch-reconnected))
               (:scan-controls (list :controls-rescanned :now now))
               (otherwise nil))))
    (multiple-value-bind (started trace)
        (retrodeck:dashboard-loop-begin-iteration
         state '(:now 1100 :wayland nil) #'handle-effect)
      (assert (equal trace
                     '((:reap-sound) (:controller-resumed)
                       (:reconnect-touch) (:render) (:present)
                       (:scan-controls :force nil))))
      (assert (not (getf (getf started :controller-guard) :suspended)))
      (assert (getf started :touch-connected-p))
      (assert (= (getf started :last-touch-reconnect-ms) 1100))
      (assert (= (getf started :last-control-scan-ms) 1100))
      (assert (string= (getf (getf started :dashboard) :status)
                       "TOUCHSCREEN RECONNECTED"))
      (multiple-value-bind (moved input-trace)
          (retrodeck:dashboard-loop-dispatch-input
           started
           '(:gamepad-actions (:right) :keyboard-actions nil
             :touch-reports nil :now 1101
             :controller-quarantined-p nil)
           #'current-layout #'handle-effect)
        (assert (= (getf (getf moved :dashboard) :game-position) 1))
        (assert (equal input-trace
                       '((:discard-touch) (:render) (:present)
                         (:cue :next))))))))

(let* ((games (runtime-test-games t))
       (state (retrodeck:dashboard-loop-initial-state games :now 1000))
       (layout (retrodeck:render-dashboard games :nes 0 "")))
  (setf (getf state :controller-guard)
        '(:edge-times (0) :suspended t :last-edge-at 100)
        (getf state :last-control-scan-ms) 1000)
  (multiple-value-bind (waiting begin-trace)
      (retrodeck:dashboard-loop-begin-iteration
       state '(:now 1099 :wayland t) (lambda (effect current)
                                      (declare (ignore effect current))))
    (assert (equal begin-trace '((:reap-sound))))
    (multiple-value-bind (blocked input-trace)
        (retrodeck:dashboard-loop-dispatch-input
         waiting
         '(:gamepad-actions (:right) :keyboard-actions nil
           :touch-reports nil :now 1100 :controller-quarantined-p nil)
         (lambda () layout)
         (lambda (effect current) (declare (ignore effect current))))
      (assert (null input-trace))
      (assert (zerop (getf (getf blocked :dashboard) :game-position)))
      (assert (getf (getf blocked :controller-guard) :suspended))
      (assert (= (getf (getf blocked :controller-guard) :last-edge-at)
                 1100)))))

(let ((state (retrodeck:dashboard-loop-initial-state nil :now 5000)))
  (setf (getf state :last-control-scan-ms) 5000)
  (flet ((ignore-effect (effect current)
           (declare (ignore effect current))))
    (multiple-value-bind (waiting trace)
        (retrodeck:dashboard-loop-begin-iteration
         state '(:now 5999 :wayland t) #'ignore-effect)
      (assert (equal trace '((:reap-sound))))
      (assert (= (getf waiting :last-control-scan-ms) 5000))
      (multiple-value-bind (due due-trace)
          (retrodeck:dashboard-loop-begin-iteration
           waiting '(:now 6000 :wayland t) #'ignore-effect)
        (assert (equal due-trace
                       '((:reap-sound) (:scan-controls :force nil))))
        (assert (= (getf due :last-control-scan-ms) 6000))
        (multiple-value-bind (rescanned rescan-trace)
            (retrodeck:dashboard-loop-begin-iteration
             due '(:now 6001 :wayland t :rescan-controls-p t)
             #'ignore-effect)
          (assert (equal rescan-trace
                         '((:reap-sound) (:scan-controls :force t))))
          (multiple-value-bind (forced force-trace)
              (retrodeck:dashboard-loop-begin-iteration
               rescanned
               '(:now 6002 :wayland t :force-control-scan-p t)
               #'ignore-effect)
            (assert (equal force-trace
                           '((:reap-sound)
                             (:scan-controls :force t))))
            (assert (= (getf forced :last-control-scan-ms) 6002))))))))

(let ((state (retrodeck:dashboard-loop-initial-state
              nil :now 9000 :touch-connected-p nil)))
  (setf (getf state :last-touch-reconnect-ms) 9000
        (getf state :last-control-scan-ms) 9999)
  (flet ((ignore-effect (effect current)
           (declare (ignore effect current))))
    (multiple-value-bind (failed failed-trace)
        (retrodeck:dashboard-loop-begin-iteration
         state '(:now 10000 :wayland nil) #'ignore-effect)
      (assert (equal failed-trace
                     '((:reap-sound) (:reconnect-touch))))
      (assert (not (getf failed :touch-connected-p)))
      (assert (= (getf failed :last-touch-reconnect-ms) 10000))
      (multiple-value-bind (throttled throttled-trace)
          (retrodeck:dashboard-loop-begin-iteration
           failed '(:now 10999 :wayland nil) #'ignore-effect)
        (assert (not (find :reconnect-touch throttled-trace :key #'first)))
        (multiple-value-bind (restored restored-trace)
            (retrodeck:dashboard-loop-begin-iteration
             throttled '(:now 11000 :wayland nil)
             (lambda (effect current)
               (declare (ignore current))
               (and (eq (first effect) :reconnect-touch)
                    '(:touch-reconnected))))
          (assert (equal restored-trace
                         '((:reap-sound) (:reconnect-touch)
                           (:render) (:present))))
          (assert (getf restored :touch-connected-p)))))))

(let* ((games (runtime-test-games))
       (state (retrodeck:dashboard-loop-initial-state games :now 7000))
       (layout (retrodeck:render-dashboard games :nes 0 "")))
  (labels ((current-layout () layout)
           (handle-effect (effect current)
             (when (eq (first effect) :render)
               (setf layout
                     (retrodeck:render-dashboard-loop-state current 7001)))
             nil))
    (multiple-value-bind (pressed press-trace)
        (retrodeck:dashboard-loop-dispatch-input
         state
         '(:gamepad-actions nil :keyboard-actions nil
           :touch-reports ((1220 420 t t nil)) :now 7000
           :controller-quarantined-p nil)
         #'current-layout #'handle-effect)
      (assert (null press-trace))
      (assert (eq (getf (getf pressed :dashboard) :pressed-target)
                  :settings))
      (multiple-value-bind (lost lost-trace)
          (retrodeck:dashboard-loop-dispatch-input
           pressed
           '(:gamepad-actions nil :keyboard-actions nil
             :touch-reports ((1220 420 nil nil t)) :touch-lost-p t
             :now 7001 :controller-quarantined-p nil)
           #'current-layout #'handle-effect)
        (assert (eq (getf lost :view) :dashboard))
        (assert (null (getf (getf lost :dashboard) :pressed-target)))
        (assert (not (getf lost :touch-connected-p)))
        (assert (string= (getf (getf lost :dashboard) :status)
                         "WAITING FOR TOUCHSCREEN"))
        (assert (equal lost-trace '((:render) (:present))))))))

(let* ((games (runtime-test-games t))
       (state (retrodeck:dashboard-loop-initial-state games :now 7100))
       (layout (retrodeck:render-dashboard games :nes 0 ""))
       (rendered-statuses nil))
  (labels ((current-layout () layout)
           (handle-effect (effect current)
             (when (eq (first effect) :render)
               (push (getf (getf current :dashboard) :status)
                     rendered-statuses)
               (setf layout
                     (retrodeck:render-dashboard-loop-state current 7101)))
             nil))
    (multiple-value-bind (pressed ignored)
        (retrodeck:dashboard-loop-dispatch-input
         state
         '(:gamepad-actions nil :keyboard-actions nil
           :touch-reports ((1220 420 t t nil)) :now 7100
           :controller-quarantined-p nil)
         #'current-layout #'handle-effect)
      (declare (ignore ignored))
      (multiple-value-bind (moved trace)
          (retrodeck:dashboard-loop-dispatch-input
           pressed
           '(:gamepad-actions (:right) :keyboard-actions nil
             :touch-reports ((1220 420 nil nil t)) :touch-lost-p t
             :now 7101 :controller-quarantined-p nil)
           #'current-layout #'handle-effect)
        (assert (= (getf (getf moved :dashboard) :game-position) 1))
        (assert (eq (getf moved :view) :dashboard))
        (assert (not (getf moved :touch-connected-p)))
        (assert (equal (nreverse rendered-statuses)
                       '("WAITING FOR TOUCHSCREEN" "")))
        (assert (equal trace
                       '((:render) (:present) (:discard-touch)
                         (:render) (:present) (:cue :next))))))))

(let* ((games (runtime-test-games))
       (network '(:ssid "LAB" :wlan-ipv4 "10.0.0.2"
                  :wireguard-ipv4 "10.8.0.2" :selector "1"))
       (state (retrodeck:dashboard-loop-initial-state games :now 1000))
       (settings (copy-list (getf state :settings)))
       (layout nil)
       (rendered-networks nil))
  (setf (getf state :view) :settings
        (getf settings :open) t
        (getf state :settings) settings
        layout (retrodeck:render-dashboard-loop-state state 3000))
  (labels ((current-layout () layout)
           (handle-effect (effect current)
             (case (first effect)
               (:network-action
                (list :network-result :network network))
               (:render
                (push (copy-tree (getf current :network)) rendered-networks)
                (setf layout
                      (retrodeck:render-dashboard-loop-state current 3000))
                nil)
               (otherwise nil))))
    (multiple-value-bind (next trace)
        (retrodeck:dashboard-loop-dispatch-input
         state
         '(:gamepad-actions (:right) :keyboard-actions nil
           :touch-reports nil :tick-now 3000 :now 3001
           :controller-quarantined-p nil)
         #'current-layout #'handle-effect)
      (assert (equal (getf next :network) network))
      (assert (eq (getf (getf next :settings) :selected) :volume-up))
      (assert (equal (nreverse rendered-networks) (list network network)))
      (assert (equal trace
                     '((:network-action) (:render) (:present)
                       (:discard-touch) (:render) (:present)
                       (:cue :next)))))))

(let* ((state (retrodeck:dashboard-loop-initial-state nil :now 8000))
       (layout-reads 0))
  (setf (getf state :view) :credits
        (getf state :credits-started-at) 8000)
  (multiple-value-bind (animated trace)
      (retrodeck:dashboard-loop-dispatch-input
       state '(:now 8040 :poll-ready-p nil)
       (lambda () (incf layout-reads) nil)
       (lambda (effect current) (declare (ignore effect current))))
    (assert (eq (getf animated :view) :credits))
    (assert (zerop layout-reads))
    (assert (equal trace '((:render) (:present))))))

(setf *fbdev-size* nil
      *wayland-size* nil)

(let ((state (retrodeck:dashboard-loop-initial-state nil :now 1))
      (runtime (retrodeck:make-dashboard-runtime)))
  (assert (not (retrodeck:dashboard-runtime-running-p runtime)))
  (assert-signals error
                  (retrodeck:dashboard-runtime-begin-iteration
                   state runtime '(:now 1)))
  (assert-signals error
                  (retrodeck:dashboard-runtime-dispatch-input
                   state runtime '(:now 1 :poll-ready-p nil)))
  (assert-signals error (retrodeck:dashboard-runtime-poll-input runtime 0)))

(dolist (fixture
         '((:keymap "20" "12" "60" "de")
           (:brightness-state "20" "12" "05" "us")
           (:maximum-zero "0" "12" "60" "us")
           (:current-over "20" "21" "60" "us")
           (:maximum-missing nil "12" "60" "us")
           (:control-write "20" "12" "60" "us" :control-write-status 0)
           (:state-write "20" "12" "60" "us" :state-write-status 0)))
  (apply #'check-dashboard-startup-storage-failure fixture))

(let* ((state (retrodeck:dashboard-loop-initial-state
               nil :brightness 60 :now 1))
       (runtime (retrodeck:make-dashboard-runtime
                 :volume-state "/tmp/volume.state"
                 :default-volume 42))
       (plan (retrodeck:settings-activation-plan
              (getf state :settings) :volume-down))
       (keymap-plan (retrodeck:settings-activation-plan
                     (getf state :settings) :keymap))
       (brightness-plan (retrodeck:settings-activation-plan
                         (getf state :settings) :brightness-up))
       (*state-file-read-result* (test-state-result "42"))
       (*state-file-write-status* 1)
       (*state-file-write-arguments* nil)
       (*error-output* (make-broadcast-stream)))
  (setf (getf plan :path) "/tmp/volume.state")
  (assert-runtime-write runtime (list :settings-action plan) state
                        '(:settings-result :succeeded-p t)
                        "/tmp/volume.state" (test-line "37"))
  (setf *state-file-write-status* 0)
  (assert-runtime-effect runtime (list :settings-action plan) state
                         '(:settings-result :succeeded-p nil))
  (setf (getf keymap-plan :path) "/tmp/keymap.state"
        *state-file-write-status* 1
        *state-file-write-arguments* nil)
  (assert-runtime-write runtime (list :settings-action keymap-plan) state
                        '(:settings-result :succeeded-p t)
                        "/tmp/keymap.state" (test-line "cz"))
  (setf *state-file-write-status* 0)
  (assert-runtime-effect runtime (list :settings-action keymap-plan) state
                         '(:settings-result :succeeded-p nil))
  (setf (getf brightness-plan :device-path) "/tmp/brightness"
        (getf brightness-plan :state-path) "/tmp/brightness.state"
        (getf runtime :brightness-maximum) 20
        *control-file-write-status* 1
        *state-file-write-status* 1
        *storage-write-trace* nil)
  (let ((result
          (runtime-effect runtime (list :settings-action brightness-plan) state)))
    (assert (equal result '(:settings-result :succeeded-p t)))
    (assert (equal (reverse *storage-write-trace*)
                   (list (list :control "/tmp/brightness" (test-line "14"))
                         (list :state "/tmp/brightness.state"
                               (test-line "70")))))
    (let ((pending (copy-list state)))
      (setf (getf pending :pending-settings-plan) brightness-plan)
      (multiple-value-bind (saved effects)
          (retrodeck:dashboard-reduce pending result)
        (assert (= (getf (getf saved :settings) :brightness) 70))
        (assert (string= (getf (getf saved :settings) :status)
                         "BRIGHTNESS 70%"))
        (assert (equal effects '((:render) (:present) (:cue :next)))))))
  (setf *control-file-write-status* 0
        *state-file-write-status* 1
        *storage-write-trace* nil)
  (let ((result
          (runtime-effect runtime (list :settings-action brightness-plan) state)))
    (assert (equal result '(:settings-result :succeeded-p nil)))
    (assert (equal (mapcar #'first (reverse *storage-write-trace*))
                   '(:control))))
  (setf *control-file-write-status* 1
        *state-file-write-status* 0
        *storage-write-trace* nil)
  (let ((result
          (runtime-effect runtime (list :settings-action brightness-plan) state)))
    (assert (equal result '(:settings-result :succeeded-p nil)))
    (assert (equal (mapcar #'first (reverse *storage-write-trace*))
                   '(:control :state)))
    (let ((pending (copy-list state)))
      (setf (getf pending :pending-settings-plan) brightness-plan)
      (multiple-value-bind (failed effects)
          (retrodeck:dashboard-reduce pending result)
        (assert (= (getf (getf failed :settings) :brightness) 60))
        (assert (string= (getf (getf failed :settings) :status)
                         "BRIGHTNESS ERROR - CHECK LOG"))
        (assert (equal effects '((:render) (:present) (:cue :next)))))))
  (setf *state-file-write-status* 1
        *state-file-write-arguments* nil
        *state-file-read-result* (test-state-result "55"))
  (assert-runtime-effect runtime '(:reload-volume) state
                         '(:child-complete :volume 55))
  (assert (null *state-file-write-arguments*))
  (let ((changed (copy-list state))
        (settings (copy-list (getf state :settings))))
    (setf (getf settings :volume) 37
          (getf changed :settings) settings
          *state-file-read-result* (test-state-result "on")
          *state-file-write-arguments* nil)
    (assert-runtime-write runtime '(:reload-volume) changed
                          '(:child-complete :volume 42)
                          "/tmp/volume.state" (test-line "42"))
    (setf *state-file-read-result* '(0)
          *state-file-write-arguments* nil)
    (assert-runtime-write runtime '(:reload-volume) changed
                          '(:child-complete :volume 42)
                          "/tmp/volume.state" (test-line "42")))
  (setf *state-file-read-result* (test-state-result "off"))
  (assert-runtime-write runtime '(:reload-volume) state
                        '(:child-complete :volume 0)
                        "/tmp/volume.state" (test-line "0"))
  (setf *state-file-read-result* (test-state-result "055"))
  (assert-runtime-effect runtime '(:reload-volume) state
                         '(:child-complete)))

(let* ((times '(101 102 103))
       (state (retrodeck:dashboard-loop-initial-state nil :now 90))
       (runtime (retrodeck:make-dashboard-runtime
                 :clock (lambda () (or (pop times) 999))))
       (*evdev-controls-dispatch-timeout* :untouched)
       (*evdev-dispatch-timeout* :untouched)
       (*input-poll-result* '(1 3 2 1 1 0 0))
       (*input-poll-arguments* nil)
       (*network-status-result*
         '("NET1" "10.0.1.11" "10.0.0.15" "CONNECTED"))
       (*network-status-path* nil)
       (*control-file-read-paths* nil)
       (*state-file-read-result* (test-state-result "37"))
       (*state-file-read-results*
        (list (cons "/mnt/data/nes-deck/state/menu-brightness.state"
                    (test-state-result "60"))
              (cons "/mnt/data/nes-deck/state/terminal-keymap.state"
                    (test-state-result "cz"))))
       (*state-file-read-path* nil)
       (*state-file-read-paths* nil))
  (with-runtime-device-fixture ()
    (with-initialized-dashboard-runtime (state runtime 90)
    (assert (= (getf (getf initialized :settings) :volume) 37))
    (assert (= (getf (getf initialized :settings) :last-audible-volume) 37))
    (assert (= (getf (getf initialized :settings) :brightness) 60))
    (assert (= (getf runtime :brightness-maximum) 20))
    (assert (string= (getf (getf initialized :settings) :keymap) "cz"))
    (assert
     (equal (reverse *control-file-read-paths*)
            '("/sys/class/backlight/display-bl/max_brightness"
              "/sys/class/backlight/display-bl/brightness")))
    (assert
     (equal (reverse *state-file-read-paths*)
            '("/mnt/data/nes-deck/state/menu-volume.state"
              "/mnt/data/nes-deck/state/menu-brightness.state"
              "/mnt/data/nes-deck/state/terminal-keymap.state")))
    (assert (equal (getf initialized :network)
                   '(:ssid "NET1" :wlan-ipv4 "10.0.1.11"
                     :wireguard-ipv4 "10.0.0.15" :selector "CONNECTED")))
    (assert (string= *network-status-path* "/var/run/deck-wifi/status"))
    (setf *network-status-result*
          '("NET2" "10.0.1.12" "10.0.0.16" "RECOVERING"))
    (assert
     (equal
      (runtime-effect runtime '(:network-action) initialized)
      '(:network-result :network
        (:ssid "NET2" :wlan-ipv4 "10.0.1.12"
         :wireguard-ipv4 "10.0.0.16" :selector "RECOVERING"))))
    (setf *evdev-controls* '((1 #x224 0) (0 15 1) (0 106 2))
          *evdev-touch-queue* '((10 20 1 1 0) (11 21 0 0 1)))
    (let ((snapshot (retrodeck:dashboard-runtime-poll-input runtime 250)))
      (assert (equal snapshot
                     '(:now 101 :tick-now 101 :poll-ready-p t
                       :touch-reports ((10 20 t t nil) (11 21 nil nil t))
                       :touch-times (101 101) :touch-lost-p t
                       :gamepad-actions (:right :system-next :confirm)
                       :keyboard-actions (:right :system-previous)
                       :rescan-controls-p t :shutdown-p nil)))
      (assert (equal *input-poll-arguments* '(0 250)))
      (assert (eq *evdev-controls-dispatch-timeout* :untouched))
      (assert (eq *evdev-dispatch-timeout* :untouched))
      (assert (= (getf runtime :now) 101)))
    (setf *input-poll-result* '(0 0 0 0 1 0 0)
          *evdev-controls* nil
          *evdev-touch-queue* nil)
    (let ((snapshot (retrodeck:dashboard-runtime-poll-input runtime 40)))
      (assert (equal snapshot
                     '(:now 102 :tick-now 102 :poll-ready-p nil
                       :touch-reports nil :touch-times nil :touch-lost-p nil
                       :gamepad-actions nil :keyboard-actions nil
                       :rescan-controls-p t :shutdown-p nil)))
      (with-runtime-dispatch (after-timeout trace) initialized runtime snapshot
        (assert (null trace))
        (assert (getf runtime :rescan-controls-p))
        (with-runtime-begin (after-scan scan-trace) after-timeout runtime '(:now 103)
          (declare (ignore after-scan))
          (assert (equal scan-trace
                         '((:reap-sound) (:scan-controls :force t))))
          (assert (not (getf runtime :rescan-controls-p))))))
    (setf *input-poll-result* '(1 1 0 0 0 0)
          *evdev-controls* nil)
    (assert-signals error (retrodeck:dashboard-runtime-poll-input runtime 0))
    (setf *input-poll-result* nil)
    (assert-signals error (retrodeck:dashboard-runtime-poll-input runtime 0))
    (retrodeck:dashboard-runtime-shutdown runtime))))

(let* ((times '(950 951 952 953 954 955))
       (state (retrodeck:dashboard-loop-initial-state nil :now 900))
       (runtime
         (retrodeck:make-dashboard-runtime
          :clock (lambda () (or (pop times) 999))))
       (*active-status* 0)
       (*input-poll-result* '(0 0 0 0 0 0 0))
       (*input-poll-arguments* nil)
       (*network-status-result* '("" "" "" "CONNECTED"))
       (*projection-status* 1)
       (*canvas-clear-status* 1)
       (*canvas-fill-status* 1))
  (with-runtime-device-fixture ()
    (with-initialized-dashboard-runtime (state runtime 900)
    (multiple-value-bind (after-normal normal-runtime normal-trace)
        (retrodeck:dashboard-runtime-run-iteration initialized runtime)
      (assert (eq normal-runtime runtime))
      (assert (equal normal-trace '((:reap-sound))))
      (assert (equal *input-poll-arguments* '(0 250)))
      (assert (= (getf runtime :now) 951))
      (let ((animated (copy-list after-normal)))
        (setf (getf animated :view) :credits
              (getf animated :credits-crawl)
              (retrodeck:make-project-credits-crawl nil)
              (getf animated :credits-started-at) 0
              (getf animated :reduced-motion) nil)
        (multiple-value-bind
              (after-animation animation-runtime animation-trace)
            (retrodeck:dashboard-runtime-run-iteration animated runtime)
          (assert (eq animation-runtime runtime))
          (assert (eq (getf after-animation :view) :credits))
          (assert (equal animation-trace
                         '((:reap-sound) (:render) (:present))))
          (assert (equal *input-poll-arguments* '(0 40)))))
      (setf *input-poll-result* '(0 0 0 0 0 1 1))
      (multiple-value-bind
            (after-shutdown shutdown-runtime shutdown-trace)
          (retrodeck:dashboard-runtime-run-iteration after-normal runtime)
        (declare (ignore after-shutdown))
        (assert (eq shutdown-runtime runtime))
        (assert (equal shutdown-trace '((:reap-sound))))
        (assert-runtime-observations :running nil :fbdev-close 1 :evdev-close 1 :controls-close 1))))))

(labels ((exercise (input times function &optional games palette)
           (let* ((state (retrodeck:dashboard-loop-initial-state games :now 90))
                  (retrodeck:*dashboard-palette*
                    (or palette retrodeck:*dashboard-palette*))
                  (runtime
                    (retrodeck:make-dashboard-runtime
                     :clock (lambda () (or (pop times) 999))))
                  (*active-status* 0)
                  (*active-count* 0)
                  (*input-poll-result* input)
                  (*input-poll-arguments* nil)
                  (*network-status-result* '("" "" "" "CONNECTED"))
                  (*control-file-read-results*
                    (list
                     (cons "/sys/class/backlight/display-bl/max_brightness"
                           (test-line "20"))
                     (cons "/sys/class/backlight/display-bl/brightness"
                           (test-line "12"))))
                  (*state-file-read-results*
                    (list
                     (cons "/mnt/data/nes-deck/state/menu-volume.state"
                           (test-state-result "37"))
                     (cons "/mnt/data/nes-deck/state/menu-brightness.state"
                           (test-state-result "60"))
                     (cons "/mnt/data/nes-deck/state/terminal-keymap.state"
                           (test-state-result "us"))))
                  (*projection-status* 1)
                  (*canvas-clear-status* 1)
                  (*canvas-glyph-status* 1)
                  (*canvas-fill-status* 1))
             (with-runtime-device-fixture ()
               (setf retrodeck::*menu-sound-input-until-ms* 0)
               (funcall function state runtime)))))
  (let* ((manifest-path (test-menu-path "games.tsv" t))
         (palette-path (test-menu-path "palette.tsv" t))
         (credits-path (test-menu-path "credits.tsv" t))
         (*regular-file-result* nil)
         (*regular-file-results*
           (list (cons manifest-path (test-file-string manifest-path))
                 (cons palette-path (test-file-string palette-path))
                 (cons credits-path (test-file-string credits-path))))
         (*regular-file-calls* nil)
         (retrodeck:*dashboard-reduced-motion-environment*
           "RETRODECK_TEST_REDUCED_MOTION_MUST_BE_MISSING"))
    (exercise
     '(0 0 0 0 0 0 0) '(100 101 102 103 104)
     (lambda (ignored-state runtime)
       (declare (ignore ignored-state))
       (multiple-value-bind
             (state candidate-runtime palette palette-loaded-p credits-loaded-p)
           (retrodeck:load-dashboard-candidate-session
            manifest-path palette-path :credits-path credits-path
            :runtime runtime)
         (assert (eq candidate-runtime runtime))
         (assert palette-loaded-p)
         (assert credits-loaded-p)
         (assert (not (getf state :reduced-motion)))
         (assert (= (length (getf (getf state :credits-crawl) :static-lines))
                    30))
         (let ((candidate-palette (copy-tree palette)))
           (setf (cdr (assoc :background candidate-palette)) #x010203)
           (multiple-value-bind (final returned-runtime traces reason)
               (retrodeck:dashboard-candidate-session-rehearse
                state runtime candidate-palette :iteration-limit 2)
             (assert (eq returned-runtime runtime))
             (assert (equal traces '(((:reap-sound)) ((:reap-sound)))))
             (assert (eq reason :limit))
             (assert (= (length (getf final :games)) 20))
             (assert (string= (getf (first (getf final :games)) :id)
                              "mario"))
             (assert (= (getf (getf final :settings) :volume) 37))
             (assert (= *canvas-clear-color* #x010203))
             (assert-runtime-observations :active-count 2 :fbdev-open 1 :fbdev-close 1
              :evdev-open 1 :evdev-close 1 :controls-close 1
              :initialized nil :running nil))))))
    (assert
     (equal (reverse *regular-file-calls*)
            (list (list manifest-path 1 65536)
                  (list palette-path 1 4096)
                  (list credits-path 1 32768)))))
  (exercise
   '(0 0 0 0 0 0 0) '(200)
   (lambda (state runtime)
     (let ((stops nil))
       (multiple-value-bind (final returned-runtime traces reason)
           (retrodeck:dashboard-runtime-rehearse
            state runtime
            :stop-predicate
            (lambda (current active iteration)
              (assert (= (getf (getf current :settings) :volume) 37))
              (assert (eq active runtime))
              (push iteration stops)
              t))
         (declare (ignore final))
         (assert (eq returned-runtime runtime))
         (assert (null traces))
         (assert (eq reason :operator-stop))
         (assert (equal stops '(0)))
         (assert (null *input-poll-arguments*))
         (assert-runtime-observations :fbdev-close 1 :evdev-close 1 :controls-close 1)))))
  (exercise
   '(0 0 0 0 0 1 0) '(300 301 302)
   (lambda (state runtime)
     (multiple-value-bind (final returned-runtime traces reason)
         (retrodeck:dashboard-runtime-rehearse state runtime)
       (declare (ignore final))
       (assert (eq returned-runtime runtime))
       (assert (equal traces '(((:reap-sound)))))
       (assert (eq reason :shutdown))
       (assert-runtime-observations :active-count 1 :fbdev-close 1 :evdev-close 1
        :controls-close 1))))
  (exercise
   nil '(400 401)
   (lambda (state runtime)
     (assert-signals error
                     (retrodeck:dashboard-runtime-rehearse state runtime))
     (assert-runtime-observations :active-count 1 :fbdev-close 1 :evdev-close 1
      :controls-close 1 :initialized nil :running nil)))
  (exercise
   '(0 0 0 0 0 0 0) '(500)
   (lambda (state runtime)
     (with-initialized-dashboard-runtime (state runtime 500)
       (assert-signals error
                       (retrodeck:dashboard-runtime-rehearse state runtime))
       (assert-runtime-observations :initialized t :running t :fbdev-close 0 :evdev-close 0
        :controls-close 0)
       (retrodeck:dashboard-runtime-shutdown runtime)
       (assert-runtime-observations :fbdev-close 1 :evdev-close 1 :controls-close 1)))))

(labels ((check-presentation
             (display expected diagnostic
              &key wayland-size fbdev-size (wayland-open 1)
                   (fbdev-open 1) adopt)
           (let ((runtime
                   (retrodeck:make-dashboard-runtime
                    :auto-presentation t :adopt-presentation adopt
                    :wayland-display display))
                 (errors (make-string-output-stream)))
             (with-runtime-device-fixture
                 (:wayland-size wayland-size :fbdev-size fbdev-size
                  :wayland-open-status wayland-open
                  :fbdev-open-status fbdev-open)
               (let ((*error-output* errors))
                 (multiple-value-bind (opened owned)
                     (retrodeck::dashboard-runtime-open-presentation runtime)
                   (let ((selected (getf runtime :wayland)))
                     (when owned
                       (retrodeck::dashboard-runtime-close-presentation runtime))
                     (assert
                      (equal
                       (list opened owned selected *wayland-open-display*
                             *wayland-open-count* *fbdev-open-count*
                             *wayland-close-count* *fbdev-close-count*)
                       expected))))))
             (let ((text (get-output-stream-string errors)))
               (assert (if diagnostic (search diagnostic text)
                           (string= text "")))))))
  (let ((runtime (retrodeck:make-dashboard-runtime
                  :auto-presentation t :wayland-display "wayland-1")))
    (assert (getf runtime :auto-presentation))
    (assert (string= (getf runtime :wayland-display) "wayland-1"))
    (assert (retrodeck::dashboard-runtime-wayland-requested-p runtime)))
  (let ((runtime (retrodeck:make-dashboard-runtime
                  :auto-presentation t :wayland-display "")))
    (assert (not (retrodeck::dashboard-runtime-wayland-requested-p runtime))))
  (assert-signals type-error
                  (retrodeck:make-dashboard-runtime
                   :auto-presentation t :wayland-display 1))
  (assert-signals error
                  (retrodeck:make-dashboard-runtime
                   :wayland t :auto-presentation t))
  (assert-signals error
                  (retrodeck:make-dashboard-runtime
                   :wayland nil :auto-presentation t))
  (check-presentation nil '(t t nil nil 0 1 0 1) nil)
  (check-presentation "wayland-1" '(t t t "wayland-1" 1 0 1 0) nil)
  (check-presentation "wayland-1" '(t t nil "wayland-1" 1 1 0 1)
                      "Wayland widget unavailable; trying fbdev" :wayland-open 0)
  (check-presentation "wayland-1" '(nil nil nil "wayland-1" 1 1 0 0)
                      "Wayland widget unavailable; trying fbdev"
                      :wayland-open 0 :fbdev-open 0)
  (check-presentation "wayland-1" '(t t t nil 0 0 1 0) nil
                      :wayland-size '(1280 480) :adopt t)
  (check-presentation "wayland-1" '(t nil nil "wayland-1" 1 0 0 0)
                      "Wayland widget unavailable; trying fbdev"
                      :fbdev-size '(1280 480) :wayland-open 0))

(let ((times '(201))
      (*input-poll-result* '(1 1 1 0 0 1 0))
      (*input-poll-arguments* nil))
  (with-dashboard-runtime-fixture
      (state runtime nil 190 190 ()
       (:wayland t :clock (lambda () (or (pop times) 999))))
      (:wayland-touch-queue '((33 44 1 1 0)) :controls '((0 28 0)))
    (assert-runtime-observations :wayland-open 1 :wayland-display :environment)
    (let ((snapshot (retrodeck:dashboard-runtime-poll-input runtime 40)))
      (assert (equal snapshot
                     '(:now 201 :tick-now 201 :poll-ready-p t
                       :touch-reports ((33 44 t t nil)) :touch-times (201)
                       :touch-lost-p nil :gamepad-actions nil
                       :keyboard-actions (:confirm)
                       :rescan-controls-p nil :shutdown-p t)))
      (assert (equal *input-poll-arguments* '(1 40)))
      (assert (zerop *evdev-open-count*))
      (retrodeck:dashboard-runtime-dispatch-input initialized runtime snapshot)
      (assert-runtime-observations :running nil :wayland-close 1 :controls-close 1))))

(assert-dashboard-runtime-initialization-failure
 10 (:auto-presentation t :wayland-display "wayland-1")
 (:wayland-open-status 0 :evdev-open-status 0)
 "Wayland widget unavailable; trying fbdev"
 :wayland-open 1 :wayland-display "wayland-1" :wayland-close 0 :wayland nil
 :fbdev-open 1 :fbdev-close 1 :evdev-open 1 :evdev-close 0
 :controls-scan 0 :controls-close 0 :fbdev-canvas 0 :layout nil
 :initialized nil :running nil)

(assert-dashboard-runtime-initialization-failure
 20 () (:controls-scan-result '(3 0)) nil
 :fbdev-open 1 :fbdev-close 1 :evdev-open 1 :evdev-close 1
 :controls-scan 1 :controls-close 1 :fbdev-canvas 0 :layout nil)

(assert-dashboard-runtime-initialization-failure
 30 (:wayland t) (:wayland-canvas-status 0) nil
 :wayland-canvas 1 :wayland-close 1 :evdev-open 0 :evdev-close 0
 :controls-scan 1 :controls-close 1 :layout nil)

(assert-dashboard-runtime-initialization-failure
 40 () (:fbdev-size '(1280 480) :fbdev-canvas-status 0) nil
 :fbdev-open 0 :fbdev-close 0 :fbdev-canvas 1
 :evdev-open 1 :evdev-close 1 :controls-scan 1 :controls-close 1
 :presentation-owned nil)

(assert-dashboard-runtime-initialization-failure
 50 (:wayland t)
 (:wayland-size '(1280 480) :wayland-canvas-status 0) nil
 :wayland-canvas 1 :wayland-close 0
 :controls-scan 1 :controls-close 1 :presentation-owned nil)

(let ((external-calls 0)
      (*network-status-path* nil))
  (with-dashboard-runtime-fixture
      (state runtime nil 55 55 ()
       (:external-effect-handler
        (lambda (effect current)
          (declare (ignore current))
          (case (first effect)
            (:network-action '(:network-result :network nil))
            (otherwise (incf external-calls) '(:external-launch))))))
      (:fbdev-size '(1280 480))
    (assert (null *network-status-path*))
    (assert (not (getf runtime :presentation-owned-p)))
    (assert (equal
             (runtime-effect runtime '(:launch (:executable "/tmp/noop")) initialized)
             '(:external-launch)))
    (assert (= external-calls 1))
    (setf (getf runtime :external-effect-handler) nil)
    (assert-signals error
      (runtime-effect runtime '(:launch (:executable "/tmp/noop")) initialized))
    (retrodeck:dashboard-runtime-shutdown runtime)
    (assert-runtime-observations :fbdev-open 0 :fbdev-close 0 :evdev-close 1 :controls-close 1)))

(with-dashboard-runtime-fixture
    (state runtime nil 56 56 () (:adopt-presentation t))
    (:fbdev-size '(1280 480))
  (assert-runtime-observations :presentation-owned t :running t)
  (assert-signals error
    (retrodeck:dashboard-runtime-initialize initialized runtime 57))
  (assert (getf runtime :presentation-owned-p))
  (retrodeck:dashboard-runtime-shutdown runtime)
  (assert-runtime-observations :fbdev-open 0 :fbdev-close 1 :evdev-close 1 :controls-close 1))

(let ((*active-status* 1) (*active-count* 0) (*play-status* 2)
      (*stop-count* 0) (*finish-count* 0)
      (retrodeck::*menu-sound-input-until-ms* 0))
  (with-dashboard-runtime-fixture
      (state runtime nil 58 58 () (:wayland t))
      (:wayland-size '(1280 480))
    (with-runtime-begin (begun trace) initialized runtime '(:now 59)
      (assert (equal trace '((:reap-sound))))
      (assert-runtime-observations :active-count 1 :sound-active t :audio-owned nil)
      (assert (null (runtime-effect runtime '(:cue :next) begun)))
      (assert-runtime-observations :sound-active t :audio-owned nil)
      (runtime-effect runtime '(:stop-sound) begun)
      (runtime-effect runtime '(:finish-sound) begun)
      (assert-runtime-observations :stop-count 0 :finish-count 0 :sound-active t)
      (retrodeck:dashboard-runtime-shutdown runtime)
      (assert-runtime-observations :stop-count 0 :finish-count 0
       :wayland-close 0 :controls-close 1))))

(let ((*active-status* 0)
      (retrodeck::*menu-sound-input-until-ms* 0))
  (with-dashboard-runtime-fixture
      (state runtime nil 0 5000 (:touch-connected-p nil) ()) ()
    (assert (getf initialized :touch-connected-p))
    (assert (= *evdev-open-count* 1))
    (with-runtime-begin (begun trace) initialized runtime '(:now 5001)
      (assert (getf begun :touch-connected-p))
      (assert (string= (getf (getf begun :dashboard) :status) ""))
      (assert (= *evdev-open-count* 1))
      (assert (equal trace '((:reap-sound)))))))

(let* ((games (runtime-test-games))
       (startup-network '(:ssid "OLD" :wlan-ipv4 "10.0.0.1"
                          :wireguard-ipv4 "" :selector "0"))
       (network '(:ssid "LAB" :wlan-ipv4 "10.0.0.2"
                  :wireguard-ipv4 "10.8.0.2" :selector "1"))
       (network-reads 0)
       (state (retrodeck:dashboard-loop-initial-state games :now 5000))
       (settings (copy-list (getf state :settings)))
       (runtime
         (retrodeck:make-dashboard-runtime
          :external-effect-handler
          (lambda (effect current)
            (declare (ignore current))
            (case (first effect)
              (:network-action
               (incf network-reads)
               (push :network-action *interaction-trace*)
               (list :network-result :network
                     (if (= network-reads 1) startup-network network)))
              (otherwise (error "Unexpected external effect ~S" effect))))))
       (*active-status* 0)
       (*record-interaction* t)
       (*interaction-trace* nil)
       (retrodeck::*menu-sound-input-until-ms* 0))
  (with-runtime-device-fixture ()
    (setf (getf state :view) :settings
        (getf settings :open) t
        (getf state :settings) settings)
  (with-initialized-dashboard-runtime (state runtime 5000)
    (assert (equal (getf initialized :network) startup-network))
    (assert (= (getf initialized :network-refreshed-at) 5000))
    (assert (= network-reads 1))
    (assert (equal (nreverse *interaction-trace*)
                   '(:network-action :render :present)))
    (setf *interaction-trace* nil)
    (with-runtime-dispatch (lost trace) initialized runtime
        '(:gamepad-actions nil :keyboard-actions nil :touch-reports nil
          :touch-lost-p t :tick-now 7000 :now 7001)
      (assert (equal (getf lost :network) network))
      (assert (= network-reads 2))
      (assert (not (getf lost :touch-connected-p)))
      (assert (= *evdev-close-count* 1))
      (assert (equal trace
                     '((:network-action) (:render) (:present)
                       (:render) (:present))))
      (assert (equal (nreverse *interaction-trace*)
                     '(:network-action :render :present :touch-close
                       :render :present)))))))

(with-dashboard-runtime-fixture
    (state runtime nil 60 60 () (:wayland t))
    (:wayland-size '(1280 480))
  (assert-runtime-observations :presentation-owned nil :controls-owned t)
  (retrodeck:dashboard-runtime-shutdown runtime)
  (assert-runtime-observations :wayland-close 0 :wayland-canvas 1
   :controls-scan 1 :controls-close 1 :running nil)
  (retrodeck:dashboard-runtime-shutdown runtime)
  (assert-runtime-observations :wayland-close 0 :controls-close 1))

(let ((*active-status* 0) (*play-status* 1) (*stop-count* 0)
      (retrodeck::*menu-sound-input-until-ms* 0))
  (with-dashboard-runtime-fixture
      (state runtime (runtime-test-games t) 70 70 () ()) ()
    (with-runtime-dispatch (moved ignored-trace) initialized runtime
        '(:gamepad-actions nil :keyboard-actions (:right)
          :touch-reports nil :now 71)
      (declare (ignore ignored-trace))
      (assert (getf runtime :audio-owned-p))
      (setf retrodeck::*menu-sound-input-until-ms* 1000)
      (with-runtime-begin (reaped reap-trace) moved runtime '(:now 72)
        (assert (equal reap-trace '((:reap-sound))))
        (assert-runtime-observations :sound-active nil :audio-owned t)
        (with-runtime-dispatch (stopped trace) reaped runtime
            '(:now 73 :poll-ready-p nil :shutdown-p t)
          (declare (ignore stopped))
          (assert (null trace))
          (assert (zerop retrodeck::*menu-sound-input-until-ms*))
          (assert-runtime-observations :stop-count 1 :controls-close 1 :evdev-close 1
           :fbdev-close 1 :layout nil :running nil)
          (retrodeck:dashboard-runtime-shutdown runtime)
          (assert-runtime-observations :stop-count 1 :controls-close 1
           :evdev-close 1 :fbdev-close 1))))))

(let ((*active-status* 0) (*play-status* 1)
      (*stop-count* 0) (*finish-count* 0)
      (retrodeck::*menu-sound-input-until-ms* 0))
  (with-dashboard-runtime-fixture
      (state runtime (runtime-test-games) 80 80 ()
       (:external-effect-handler
        (lambda (effect current)
          (declare (ignore current))
          (case (first effect)
            (:network-action '(:network-result :network nil))
            (:launch
             '(:child-returned :shutdown t
               :result (:started t :exited-for-touch nil
                        :exit-code nil :signal nil :error nil)))
            (otherwise (error "Unexpected external effect ~S" effect)))))) ()
    (with-runtime-dispatch (stopped trace) initialized runtime
        '(:gamepad-actions nil :keyboard-actions (:confirm)
          :touch-reports nil :now 81)
      (assert (null (getf stopped :active-launch)))
      (assert-runtime-observations :running nil :finish-count 1 :stop-count 0
       :controls-scan 1 :controls-close 1 :evdev-close 1 :fbdev-close 1)
      (assert (equal (mapcar #'first trace)
                     '(:discard-touch :cue :render :present :finish-sound
                       :close-controls :launch :stop-loop))))))

(let ((*active-status* 1) (*active-count* 0) (*play-status* 1)
      (retrodeck::*menu-sound-input-until-ms* 0))
  (with-dashboard-runtime-fixture
      (state runtime (runtime-test-games t) 100 100 ()
       (:volume-state "/tmp/volume.state")) ()
    (assert-runtime-observations :fbdev-open 1 :evdev-open 1 :controls-scan 1 :fbdev-canvas 1)
    (assert (= (getf initialized :last-control-scan-ms) 100))
    (with-runtime-begin (begun begin-trace) initialized runtime '(:now 150)
      (assert (equal begin-trace '((:reap-sound))))
      (assert (= *active-count* 1))
      (assert (retrodeck:dashboard-runtime-controller-quarantined-p
               runtime 151))
      (with-runtime-dispatch (blocked blocked-trace) begun runtime
          '(:gamepad-actions (:right) :keyboard-actions nil
            :touch-reports nil :now 151)
        (assert (null blocked-trace))
        (assert (zerop (getf (getf blocked :dashboard) :game-position)))
        (assert (= *active-count* 1))
        (with-runtime-dispatch (moved move-trace) blocked runtime
            '(:gamepad-actions nil :keyboard-actions (:right)
              :touch-reports nil :rescan-controls-p t :now 152)
          (assert (= (getf (getf moved :dashboard) :game-position) 1))
          (assert-runtime-observations :active-count 1 :fbdev-canvas 2)
          (assert (equal move-trace
                         '((:discard-touch) (:render) (:present)
                           (:cue :next))))
          (setf *active-status* 0)
          (with-runtime-begin (rescanned rescan-trace) moved runtime '(:now 153)
            (assert (= (getf rescanned :last-control-scan-ms) 153))
            (assert-runtime-observations :active-count 2 :controls-scan 2)
            (assert (equal rescan-trace
                           '((:reap-sound)
                             (:scan-controls :force t))))))))
    (setf *active-status* 0
          retrodeck::*menu-sound-input-until-ms* 0)))

(let ((external-trace nil) (clock-now 2002)
      (*active-status* 0) (*active-count* 0) (*play-status* 1)
      (*finish-count* 0) (retrodeck::*menu-sound-input-until-ms* 0))
  (with-dashboard-runtime-fixture
      (state runtime (runtime-test-games) 2000 2000 ()
       (:volume-state "/tmp/volume.state"
        :clock (lambda () clock-now)
        :external-effect-handler
        (lambda (effect current)
          (declare (ignore current))
          (case (first effect)
            (:network-action '(:network-result :network nil))
            (:launch
             (push :launch external-trace)
             (setf clock-now 5000)
             '(:child-returned
               :result (:started t :exited-for-touch nil
                        :exit-code 0 :signal nil :error nil)))
            (:reload-volume
             (push :reload-volume external-trace)
             '(:child-complete :volume 47))
            (otherwise (error "Unexpected external effect ~S" effect)))))) ()
    (with-runtime-begin (begun begin-trace) initialized runtime '(:now 2001)
      (assert (equal begin-trace '((:reap-sound))))
      (with-runtime-dispatch (finished trace) begun runtime
          '(:gamepad-actions (:confirm) :keyboard-actions nil
            :touch-reports nil :now 2002)
        (assert (null (getf finished :active-launch)))
        (assert (= (getf finished :last-control-scan-ms) 5000))
        (assert (= (getf (getf finished :settings) :volume) 47))
        (assert (string= (getf (getf finished :dashboard) :status)
                         "ALPHA EXITED"))
        (assert (equal (nreverse external-trace) '(:launch :reload-volume)))
        (assert-runtime-observations :finish-count 1 :controls-close 1 :controls-scan 2
         :fbdev-open 2 :fbdev-canvas 3)
        (assert (not (retrodeck:dashboard-runtime-controller-quarantined-p
                      runtime 2002)))
        (assert (equal (mapcar #'first trace)
                       '(:discard-touch :cue :render :present :finish-sound
                         :close-controls :launch :scan-controls
                         :open-presentation :reload-volume
                         :render :present)))
        (with-runtime-begin (post-launch post-trace) finished runtime '(:now 5001)
          (assert (= (getf post-launch :last-control-scan-ms) 5000))
          (assert (equal post-trace '((:reap-sound))))
          (assert (= *evdev-controls-scan-count* 2)))))
    (setf retrodeck::*menu-sound-input-until-ms* 0)))

(let* ((games (runtime-test-games))
       (external-trace nil)
       (runtime
         (retrodeck:make-dashboard-runtime
          :external-effect-handler
          (lambda (effect current)
            (declare (ignore current))
            (case (first effect)
              (:network-action '(:network-result :network nil))
              (:settings-action
               (push :settings-action external-trace)
               '(:settings-result :succeeded-p t))
              (otherwise (error "Unexpected external effect ~S" effect))))))
       (state (retrodeck:dashboard-loop-initial-state games :now 3000))
       (settings (copy-list (getf state :settings))))
  (setf (getf state :view) :settings
        (getf settings :open) t
        (getf state :settings) settings
        *active-status* 0
        *play-status* 0
        *fbdev-canvas-count* 0
        retrodeck::*menu-sound-input-until-ms* 0)
  (with-initialized-dashboard-runtime (state runtime 3000)
    (with-runtime-begin (begun ignored-trace) initialized runtime '(:now 3001)
      (declare (ignore ignored-trace))
      (with-runtime-dispatch (failed trace) begun runtime
          '(:gamepad-actions (:confirm) :keyboard-actions nil
            :touch-reports nil :now 3002)
        (assert (= (getf (getf failed :settings) :volume) 37))
        (assert (string= (getf (getf failed :settings) :status)
                         "VOLUME SAVED; CONFIRMATION TONE FAILED"))
        (assert (equal (nreverse external-trace) '(:settings-action)))
        (assert (= *fbdev-canvas-count* 3))
        (assert (equal (mapcar #'first trace)
                       '(:discard-touch :settings-action :render :present
                         :cue :render :present))))))
  (setf *play-status* 1))

(let ((*active-status* 0) (*active-count* 0)
      (retrodeck::*menu-sound-input-until-ms* 0))
  (with-dashboard-runtime-fixture
      (state runtime (runtime-test-games) 4000 4000 () ()) ()
    (with-runtime-dispatch (lost lost-trace) initialized runtime
        '(:gamepad-actions nil :keyboard-actions nil
          :touch-reports nil :touch-lost-p t :now 4001)
      (assert (not (getf lost :touch-connected-p)))
      (assert (= *evdev-close-count* 1))
      (assert (equal lost-trace '((:render) (:present))))
      (with-runtime-begin (restored reconnect-trace) lost runtime '(:now 4002)
        (assert (getf restored :touch-connected-p))
        (assert (= *evdev-open-count* 2))
        (assert (equal reconnect-trace
                       '((:reap-sound) (:reconnect-touch)
                         (:render) (:present))))
        (with-runtime-dispatch (stopped stop-trace) restored runtime
            '(:now 4003 :poll-ready-p nil :shutdown-p t)
          (declare (ignore stopped))
          (assert (null stop-trace))
          (assert (not (retrodeck:dashboard-runtime-running-p runtime))))))))

(format t "Lisp policy tests passed.~%")
