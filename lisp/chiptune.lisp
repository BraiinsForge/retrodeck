(in-package #:retrodeck)

(defconstant +chiptune-block-nanoseconds+ 16666666)

(defparameter *chiptune-colors*
  '((:background . #x000000) (:orange . #xfe6c27) (:active . #x4d372d)
    (:text . #xffffff) (:green . #x87af87) (:red . #xaf8787)
    (:muted . #x969696) (:indicator . #x6c6c6c)))

(defparameter *chiptune-controls*
  '((:close 554 3 62 34) (:playback-mode 113 177 92 34)
    (:previous-file 215 177 92 34) (:pause 317 177 92 34)
    (:next-file 419 177 92 34)))

(defun chiptune-rgb565-color (color)
  (check-type color (integer 0 16777215))
  (let ((red (ldb (byte 5 19) color))
        (green (ldb (byte 6 10) color))
        (blue (ldb (byte 5 3) color)))
    (logior (ash (floor (* red 255) 31) 16)
            (ash (floor (* green 255) 63) 8)
            (floor (* blue 255) 31))))

(defun chiptune-color (role)
  (chiptune-rgb565-color
   (or (cdr (assoc role *chiptune-colors*))
       (error "Unknown chiptune color ~S" role))))

(defun chiptune-control (name)
  (or (cdr (assoc name *chiptune-controls*))
      (error "Unknown chiptune control ~S" name)))

(defun chiptune-fill-source-rect (x y width height color)
  (fill-canvas-area (+ 16 (* 2 x)) (+ 16 (* 2 y))
                    (* 2 width) (* 2 height) color))

(defun chiptune-stroke-source-rect (x y width height thickness color)
  (stroke-canvas-rect (+ 16 (* 2 x)) (+ 16 (* 2 y))
                      (* 2 width) (* 2 height) (* 2 thickness) color))

(defun chiptune-draw-source-text (x y text scale color)
  (draw-text (+ 16 (* 2 x)) (+ 16 (* 2 y)) text (* 2 scale) color))

(defun chiptune-draw-centered-source-text (y text scale color)
  (chiptune-draw-source-text
   (max 0 (floor (- 624 (bitmap-text-width text scale)) 2))
   y text scale color))

(defun chiptune-draw-panel (x y width height fill border)
  (draw-pixel-panel (+ 16 (* 2 x)) (+ 16 (* 2 y))
                    (* 2 width) (* 2 height) fill border 4))

(defun chiptune-draw-close-icon (bounds color)
  (destructuring-bind (x y width height) bounds
    (let ((center-x (+ x (floor width 2)))
          (center-y (+ y (floor height 2))))
      (loop for offset from -8 to 8 by 2
            always (and (chiptune-fill-source-rect
                         (+ center-x offset) (+ center-y offset) 2 2 color)
                        (chiptune-fill-source-rect
                         (+ center-x offset) (- center-y offset) 2 2 color))))))

(defun chiptune-draw-pixel-line
    (from-x from-y to-x to-y thickness color)
  (let ((delta-x (abs (- to-x from-x)))
        (step-x (if (< from-x to-x) 1 -1))
        (delta-y (- (abs (- to-y from-y))))
        (step-y (if (< from-y to-y) 1 -1)))
    (loop with error = (+ delta-x delta-y)
          always (chiptune-fill-source-rect
                  from-x from-y thickness thickness color)
          until (and (= from-x to-x) (= from-y to-y))
          do (let ((doubled-error (* error 2)))
               (when (>= doubled-error delta-y)
                 (incf error delta-y)
                 (incf from-x step-x))
               (when (<= doubled-error delta-x)
                 (incf error delta-x)
                 (incf from-y step-y))))))

(defun chiptune-draw-arrow-head (point-x point-y points-right color)
  (let ((direction (if points-right -1 1)))
    (and (chiptune-draw-pixel-line
          point-x point-y (+ point-x (* direction 4)) (- point-y 4) 1 color)
         (chiptune-draw-pixel-line
          point-x point-y (+ point-x (* direction 4)) (+ point-y 4) 1 color))))

(defun chiptune-draw-transport-triangle
    (center-x center-y points-right color)
  (loop for row from -6 to 6 by 2
        for width = (- 14 (* (abs row) 2))
        for left = (if points-right (- center-x 6) (- (+ center-x 6) width))
        always (chiptune-fill-source-rect left (+ center-y row) width 2 color)))

(defun chiptune-draw-previous-icon (bounds color)
  (destructuring-bind (x y width height) bounds
    (let ((center-x (+ x (floor width 2)))
          (center-y (+ y (floor height 2))))
      (and (chiptune-fill-source-rect (- center-x 10) (- center-y 7)
                                      2 14 color)
           (chiptune-draw-transport-triangle
            (1+ center-x) center-y nil color)))))

(defun chiptune-draw-next-icon (bounds color)
  (destructuring-bind (x y width height) bounds
    (let ((center-x (+ x (floor width 2)))
          (center-y (+ y (floor height 2))))
      (and (chiptune-draw-transport-triangle
            (1- center-x) center-y t color)
           (chiptune-fill-source-rect (+ center-x 8) (- center-y 7)
                                      2 14 color)))))

(defun chiptune-draw-pause-icon (bounds paused color)
  (destructuring-bind (x y width height) bounds
    (let ((center-x (+ x (floor width 2)))
          (center-y (+ y (floor height 2))))
      (if paused
          (chiptune-draw-transport-triangle (1- center-x) center-y t color)
          (and (chiptune-fill-source-rect
                (- center-x 5) (- center-y 7) 3 14 color)
               (chiptune-fill-source-rect
                (+ center-x 2) (- center-y 7) 3 14 color))))))

(defun chiptune-draw-loop-icon (bounds one color)
  (destructuring-bind (x y width height) bounds
    (let ((center-x (+ x (floor width 2)))
          (center-y (+ y (floor height 2))))
      (and (chiptune-draw-pixel-line
            (- center-x 11) (- center-y 5) (+ center-x 9) (- center-y 5) 1 color)
           (chiptune-draw-arrow-head (+ center-x 11) (- center-y 5) t color)
           (chiptune-draw-pixel-line
            (+ center-x 11) (+ center-y 5) (- center-x 9) (+ center-y 5) 1 color)
           (chiptune-draw-arrow-head (- center-x 11) (+ center-y 5) nil color)
           (or (not one)
               (chiptune-draw-source-text
                (- center-x 2) (- center-y 3) "1" 1 color))))))

(defun chiptune-draw-shuffle-icon (bounds color)
  (destructuring-bind (x y width height) bounds
    (let ((center-x (+ x (floor width 2)))
          (center-y (+ y (floor height 2))))
      (and (chiptune-draw-pixel-line
            (- center-x 12) (- center-y 5) (- center-x 7) (- center-y 5) 1 color)
           (chiptune-draw-pixel-line
            (- center-x 7) (- center-y 5) (+ center-x 6) (+ center-y 5) 1 color)
           (chiptune-draw-pixel-line
            (- center-x 12) (+ center-y 5) (- center-x 7) (+ center-y 5) 1 color)
           (chiptune-draw-pixel-line
            (- center-x 7) (+ center-y 5) (+ center-x 6) (- center-y 5) 1 color)
           (chiptune-draw-pixel-line
            (+ center-x 6) (- center-y 5) (+ center-x 10) (- center-y 5) 1 color)
           (chiptune-draw-pixel-line
            (+ center-x 6) (+ center-y 5) (+ center-x 10) (+ center-y 5) 1 color)
           (chiptune-draw-arrow-head (+ center-x 12) (- center-y 5) t color)
           (chiptune-draw-arrow-head (+ center-x 12) (+ center-y 5) t color)))))

(defun chiptune-draw-playback-mode-icon (mode color)
  (ecase mode
    (:loop-all (chiptune-draw-loop-icon
                (chiptune-control :playback-mode) nil color))
    (:loop-one (chiptune-draw-loop-icon
                (chiptune-control :playback-mode) t color))
    (:shuffle (chiptune-draw-shuffle-icon
               (chiptune-control :playback-mode) color))))

(defun chiptune-draw-file-indicators (file-index file-count orange inactive)
  (when (plusp file-count)
    (let* ((visible (min file-count 40))
           (row-width (+ (* visible 6) (* (1- visible) 4)))
           (first (if (> file-count visible)
                      (min (if (> file-index (floor visible 2))
                               (- file-index (floor visible 2)) 0)
                           (- file-count visible))
                      0)))
      (loop for index below visible
            for x from (floor (- 624 row-width) 2) by 10
            always (chiptune-stroke-source-rect
                    x 166 6 4 1
                    (if (= (+ first index) file-index) orange inactive))))))

(defun chiptune-valid-visual-p (visual)
  (or (null visual)
      (and (typep visual 'sequence) (= (length visual) 1470)
           (every (lambda (sample) (typep sample '(signed-byte 16))) visual))))

(defun chiptune-ogg-path-p (path)
  (let ((length (length path)))
    (and (>= length 4) (string-equal path ".ogg" :start1 (- length 4)))))

(defun chiptune-compose-metadata (path native track-index)
  (unless (and (listp native) (= (length native) 6)
               (every #'stringp (subseq native 0 4))
               (typep (fifth native) '(integer -1 *))
               (typep (sixth native) '(integer 1 *)))
    (error "Malformed native chiptune metadata ~S" native))
  (destructuring-bind (title game author system length track-count) native
    (unless (< track-index track-count)
      (error "Chiptune track ~D is out of range" track-index))
    (list :title (if (plusp (length title)) title (chiptune-base-name path))
          :subtitle (if (and (plusp (length game)) (plusp (length author)))
                        (concatenate 'string game " - " author)
                        (if (plusp (length game)) game author))
          :system (if (chiptune-ogg-path-p path) "OGG VORBIS" system)
          :length length :track-index track-index :track-count track-count)))

(defun open-chiptune-file (path)
  (check-type path string)
  (let ((result (retrodeck.native:chiptune-open (native-path-string path))))
    (when result
      (chiptune-compose-metadata path result 0))))

(defun start-chiptune-track (path track)
  (check-type path string)
  (check-type track (integer 0 *))
  (let ((result (retrodeck.native:chiptune-start-track track)))
    (when result
      (chiptune-compose-metadata path result track))))

(defun chiptune-decode-pcm (pcm)
  (unless (and (stringp pcm) (= (length pcm) 2940))
    (error "Native chiptune PCM must contain 2940 bytes"))
  (let ((visual (make-array 1470 :element-type '(signed-byte 16))))
    (dotimes (index 1470 visual)
      (let* ((offset (* index 2))
             (unsigned (logior (char-code (char pcm offset))
                               (ash (char-code (char pcm (1+ offset))) 8))))
        (setf (aref visual index)
              (if (logbitp 15 unsigned) (- unsigned #x10000) unsigned))))))

(defun step-chiptune-file ()
  (let ((result (retrodeck.native:chiptune-step)))
    (when result
      (unless (and (listp result) (= (length result) 4)
                   (stringp (first result))
                   (member (second result) '(0 1))
                   (typep (third result) '(integer 0 735))
                   (typep (fourth result) '(integer 0 *)))
        (error "Malformed native chiptune snapshot ~S" result))
      (values (chiptune-decode-pcm (first result))
              (= (second result) 1) (third result) (fourth result)
              (first result)))))

(defun open-chiptune-audio (volume)
  (check-type volume (integer 0 100))
  (= (retrodeck.native:chiptune-audio-open volume) 1))

(defun write-chiptune-audio (pcm)
  (unless (and (stringp pcm) (= (length pcm) 2940))
    (error "Chiptune audio needs one 2940-byte PCM block"))
  (= (retrodeck.native:chiptune-audio-write pcm) 1))

(defun close-chiptune-audio ()
  (= (retrodeck.native:chiptune-audio-close) 1))

(defun rewind-chiptune-file ()
  (= (retrodeck.native:chiptune-rewind) 1))

(defun close-chiptune-file ()
  (= (retrodeck.native:chiptune-close) 1))

(defun make-chiptune-render-state
    (&key ready (title "") (subtitle "") (system "") (position 0) (length -1)
       (file-index 0) (file-count 0) (track-index 0) (track-count 0)
       paused (playback-mode :loop-all) (volume 42) (status "") visual)
  (check-type ready boolean)
  (check-type title string)
  (check-type subtitle string)
  (check-type system string)
  (check-type position (integer 0 *))
  (check-type length (integer -1 *))
  (check-type file-index (integer 0 *))
  (check-type file-count (integer 0 *))
  (check-type track-index (integer 0 *))
  (check-type track-count (integer 0 *))
  (check-type paused boolean)
  (check-type volume (integer 0 100))
  (check-type status string)
  (unless (member playback-mode '(:loop-all :loop-one :shuffle))
    (error "Unknown chiptune playback mode ~S" playback-mode))
  (unless (chiptune-valid-visual-p visual)
    (error "Chiptune visual must contain 1470 signed 16-bit samples"))
  (when (and ready
             (or (zerop file-count) (>= file-index file-count)
                 (zerop track-count) (>= track-index track-count)))
    (error "Ready chiptune state has invalid file or track selection"))
  (list :ready ready :title title :subtitle subtitle :system system
        :position position :length length :file-index file-index
        :file-count file-count :track-index track-index :track-count track-count
        :paused paused :playback-mode playback-mode :volume volume
        :status status :visual visual))

(defun chiptune-draw-waveform (visual background muted orange)
  (and (chiptune-fill-source-rect 96 84 432 44 background)
       (chiptune-fill-source-rect 96 105 432 1 muted)
       (or (null visual)
           (loop for x below 432
                 for frame = (floor (* x 735) 432)
                 for mixed = (truncate (+ (elt visual (* frame 2))
                                          (elt visual (1+ (* frame 2)))) 2)
                 for height = (min 20 (floor (abs mixed) 1050))
                 always (chiptune-fill-source-rect
                         (+ 96 x) (if (minusp mixed) 106 (- 105 height))
                         1 (max 1 height) orange)))))

(defun render-chiptune (state)
  (let* ((checked (apply #'make-chiptune-render-state state))
         (ready (getf checked :ready))
         (background (chiptune-color :background))
         (orange (chiptune-color :orange))
         (active (chiptune-color :active))
         (text (chiptune-color :text))
         (green (chiptune-color :green))
         (red (chiptune-color :red))
         (muted (chiptune-color :muted))
         (indicator (chiptune-color :indicator))
         (volume (getf checked :volume)))
    (and (clear-canvas #x000000)
         (chiptune-fill-source-rect 0 0 624 224 background)
         (chiptune-draw-panel 236 4 152 29 active orange)
         (chiptune-draw-centered-source-text 12 "CHIPTUNES" 1 text)
         (chiptune-draw-close-icon (chiptune-control :close) text)
         (chiptune-draw-source-text
          8 14 (if (plusp volume) (format nil "VOL ~D" volume) "VOL OFF")
          1 (if (plusp volume) green red))
         (if ready
             (and (chiptune-draw-panel 78 42 468 120 active orange)
                  (chiptune-draw-centered-source-text
                   50 (chiptune-display-text (getf checked :title) 45) 2 text)
                  (chiptune-draw-centered-source-text
                   70 (chiptune-display-text (getf checked :subtitle) 72) 1 muted)
                  (chiptune-draw-waveform
                   (getf checked :visual) background muted orange)
                  (let* ((position (getf checked :position))
                         (length (getf checked :length))
                         (progress (if (plusp length)
                                       (max 0 (min 432
                                                   (truncate (* position 432)
                                                             length))) 0))
                         (end-time (if (plusp length)
                                       (chiptune-format-time length) "--:--"))
                         (details
                           (format nil "~A  FILE ~D/~D  TRACK ~D/~D"
                                   (chiptune-display-text
                                    (getf checked :system) 18)
                                   (1+ (getf checked :file-index))
                                   (getf checked :file-count)
                                   (1+ (getf checked :track-index))
                                   (getf checked :track-count))))
                    (and (chiptune-fill-source-rect 96 134 432 3 background)
                         (or (zerop progress)
                             (chiptune-fill-source-rect
                              96 134 progress 3 green))
                         (chiptune-draw-source-text
                          96 143 (chiptune-format-time position) 1 text)
                         (chiptune-draw-source-text
                          (- 528 (bitmap-text-width end-time 1))
                          143 end-time 1 text)
                         (chiptune-draw-centered-source-text
                          143 (chiptune-display-text details 56) 1 muted)
                         (chiptune-draw-file-indicators
                          (getf checked :file-index) (getf checked :file-count)
                          orange indicator))))
             (and (chiptune-draw-panel 78 42 468 120 active orange)
                  (chiptune-draw-centered-source-text
                   72 "NO CHIPTUNES FOUND" 2 text)
                  (chiptune-draw-centered-source-text
                   103 (chiptune-display-text (getf checked :status) 72) 1 muted)
                  (chiptune-draw-centered-source-text
                   126 "AY GBS GYM HES KSS NSF NSFE OGG SAP SPC VGM VGZ"
                   1 text)))
         (chiptune-draw-playback-mode-icon
          (getf checked :playback-mode) text)
         (chiptune-draw-previous-icon
          (chiptune-control :previous-file) text)
         (chiptune-draw-pause-icon
          (chiptune-control :pause) (getf checked :paused) text)
         (chiptune-draw-next-icon (chiptune-control :next-file) text))))

(defparameter *chiptune-gamepad-controls*
  '((#x001 . :back) (#x002 . :back)
    (#x004 . :toggle-pause) (#x008 . :toggle-pause)
    (#x010 . :previous-track) (#x020 . :next-track)
    (#x080 . :playback-mode)
    (#x100 . :previous-file) (#x200 . :next-file)
    (#x400 . :volume-up) (#x800 . :volume-down)))

(defun chiptune-control-commands (report)
  (check-type report list)
  (ecase (getf report :kind)
    (:keyboard nil)
    (:gamepad
     (let ((edges (getf report :edges)))
       (check-type edges (integer 1 4095))
       (remove-duplicates
        (loop for (mask . command) in *chiptune-gamepad-controls*
              when (logtest mask edges) collect command)
        :test #'eq)))))

(defun chiptune-touch-command (action)
  (ecase action
    (:close :back)
    (:pause :toggle-pause)
    ((:previous-file :next-file :playback-mode) action)))

(defun chiptune-next-playback-mode (mode)
  (ecase mode
    (:loop-all :loop-one)
    (:loop-one :shuffle)
    (:shuffle :loop-all)))

(defun chiptune-next-random (state)
  "Advance the C++ player's xorshift32 shuffle generator."
  (check-type state (unsigned-byte 32))
  (let ((x (if (zerop state) #x6d2b79f5 state)))
    (setf x (logand #xffffffff (logxor x (ash x 13)))
          x (logxor x (ash x -17))
          x (logand #xffffffff (logxor x (ash x 5))))
    x))

(defun chiptune-file-candidates (count current direction)
  "File indices to try for previous or next navigation, wrapping to CURRENT."
  (check-type count (integer 1 *))
  (check-type current (integer 0 *))
  (loop for attempt from 1 to count
        collect (if (minusp direction)
                    (mod (- (+ current count) (mod attempt count)) count)
                    (mod (+ current attempt) count))))

(defun chiptune-shuffle-candidates (count current random)
  "Shuffled file indices to try, skipping CURRENT while alternatives exist."
  (check-type count (integer 1 *))
  (check-type current (integer 0 *))
  (check-type random (unsigned-byte 32))
  (let ((offset (if (> count 1) (1+ (mod random (1- count))) 0)))
    (loop for attempt below count
          for candidate = (mod (+ current offset attempt) count)
          unless (and (> count 1) (= candidate current))
            collect candidate)))

(defun chiptune-advance-plan (mode track-index track-count)
  "Decide what follows a finished track under the given playback mode."
  (check-type track-index (integer 0 *))
  (check-type track-count (integer 1 *))
  (ecase mode
    (:loop-one '(:restart))
    (:shuffle '(:shuffle))
    (:loop-all (if (< (1+ track-index) track-count)
                   (list :track (1+ track-index))
                   '(:next-file)))))

(defun chiptune-volume-step (volume direction)
  (check-type volume (integer 0 100))
  (if (minusp direction)
      (if (>= volume 5) (- volume 5) 0)
      (min 100 (+ volume 5))))

(defun chiptune-touch-action (logical-x logical-y)
  (check-type logical-x integer)
  (check-type logical-y integer)
  (let ((x (truncate (- logical-x 16) 2))
        (y (truncate (- logical-y 16) 2)))
    (loop for name in '(:close :previous-file :pause :next-file :playback-mode)
          for (left top width height) = (chiptune-control name)
          when (and (<= left x) (< x (+ left width))
                    (<= top y) (< y (+ top height)))
            return name)))

(defun chiptune-runtime-volume (&optional (text nil supplied-p))
  (handler-case
      (if supplied-p (parse-dashboard-inherited-volume text)
          (dashboard-inherited-volume))
    (error ()
      (format *error-output*
              "chiptune-deck: volume must be an integer from 0 through 100; playing muted~%")
      (finish-output *error-output*)
      0)))

(defun make-chiptune-runtime
    (&key (directory "/mnt/data/chiptunes")
       (presentation (dashboard-environment-value "RETRO_DECK_PRESENTATION"))
       (wayland-display
         (dashboard-environment-value *dashboard-wayland-display-environment*))
       (volume-state (dashboard-environment-value "RETRO_DECK_VOLUME_STATE"))
       (clock #'monotonic-nanoseconds))
  (check-type directory string)
  (check-type volume-state (or null string))
  (check-type clock function)
  (let* ((wayland (ten-seconds-wayland-requested-p presentation wayland-display))
         (runtime (make-dashboard-runtime :wayland wayland
                                          :wayland-display wayland-display
                                          :default-volume 42 :clock clock)))
    (setf (getf runtime :chiptune-directory) directory
          (getf runtime :chiptune-volume-state)
          (and volume-state (plusp (length volume-state)) volume-state)
          (getf runtime :next-block-at) 0
          (getf runtime :chiptune-blocks) 0
          (getf runtime :volume) nil
          (getf runtime :dirty) t)
    runtime))

(defun make-chiptune-player-state (files &key (random-seed 0) (status ""))
  (check-type files list)
  (check-type random-seed (unsigned-byte 32))
  (check-type status string)
  (list :files files :file-index 0 :metadata nil :paused nil
        :playback-mode :loop-all :random-state random-seed
        :status status :visual nil :position 0 :pending-pcm nil))

(defun chiptune-state-path (state)
  (elt (getf state :files) (getf state :file-index)))

(defun chiptune-state-note-track (state metadata)
  (setf (getf state :metadata) metadata
        (getf state :paused) nil
        (getf state :visual) nil
        (getf state :position) 0
        (getf state :pending-pcm) nil)
  metadata)

(defun chiptune-open-index (state index)
  (let ((metadata (open-chiptune-file (elt (getf state :files) index))))
    (when metadata
      (setf (getf state :file-index) index)
      (chiptune-state-note-track state metadata))))

(defun chiptune-open-any (state candidates)
  (loop for index in candidates
          thereis (chiptune-open-index state index)))

(defun chiptune-state-start-track (state track)
  (let ((metadata (start-chiptune-track (chiptune-state-path state) track)))
    (when metadata
      (chiptune-state-note-track state metadata))))

(defun chiptune-state-draw-random (state)
  (setf (getf state :random-state)
        (chiptune-next-random (getf state :random-state))))

(defun chiptune-shuffle-jump (state)
  (let* ((count (length (getf state :files)))
         (random (chiptune-state-draw-random state))
         (opened (or (chiptune-open-any
                      state (chiptune-shuffle-candidates
                             count (getf state :file-index) random))
                     (chiptune-open-index state (getf state :file-index)))))
    (when opened
      (let ((track-count (getf opened :track-count)))
        (if (> track-count 1)
            (chiptune-state-start-track
             state (mod (chiptune-state-draw-random state) track-count))
            opened)))))

(defun chiptune-playback-failed (state reason)
  (close-chiptune-file)
  (setf (getf state :metadata) nil
        (getf state :pending-pcm) nil
        (getf state :visual) nil
        (getf state :status) (format nil "PLAYBACK ERROR: ~A" reason))
  nil)

(defun chiptune-state-advance (state)
  (let* ((metadata (getf state :metadata))
         (plan (chiptune-advance-plan (getf state :playback-mode)
                                      (getf metadata :track-index)
                                      (getf metadata :track-count))))
    (or (ecase (first plan)
          (:restart (and (rewind-chiptune-file)
                         (setf (getf state :position) 0)
                         metadata))
          (:track (chiptune-state-start-track state (second plan)))
          (:shuffle (chiptune-shuffle-jump state))
          (:next-file
           (chiptune-open-any
            state (chiptune-file-candidates (length (getf state :files))
                                            (getf state :file-index) 1))))
        (chiptune-playback-failed state "cannot advance to the next track"))))

(defun chiptune-runtime-set-volume (runtime requested)
  (if (open-chiptune-audio requested)
      (let ((path (getf runtime :chiptune-volume-state)))
        (setf (getf runtime :volume) requested
              (getf runtime :audio-owned-p) t
              (getf runtime :dirty) t)
        (when (and path (not (save-dashboard-volume-state path requested)))
          (format *error-output* "chiptune-deck: cannot save volume state~%")
          (finish-output *error-output*))
        t)
      (progn
        (format *error-output* "chiptune-deck: cannot change volume~%")
        (finish-output *error-output*)
        nil)))

(defun chiptune-apply-commands (state runtime commands)
  (flet ((dirty () (setf (getf runtime :dirty) t))
         (navigate (direction)
           (let ((count (length (getf state :files))))
             (when (and (plusp count)
                        (chiptune-open-any
                         state (chiptune-file-candidates
                                count (getf state :file-index) direction)))
               (setf (getf runtime :dirty) t))))
         (switch-track (direction)
           (let ((metadata (getf state :metadata)))
             (when (and metadata
                        (not (chiptune-ogg-path-p (chiptune-state-path state))))
               (let* ((count (getf metadata :track-count))
                      (next (mod (+ (getf metadata :track-index) count direction)
                                 count)))
                 (when (chiptune-state-start-track state next)
                   (setf (getf runtime :dirty) t)))))))
    (when (member :back commands) (setf (getf runtime :running) nil))
    (when (member :previous-file commands) (navigate -1))
    (when (member :next-file commands) (navigate 1))
    (when (member :previous-track commands) (switch-track -1))
    (when (member :next-track commands) (switch-track 1))
    (when (member :toggle-pause commands)
      (setf (getf state :paused) (not (getf state :paused)))
      (dirty))
    (when (member :playback-mode commands)
      (setf (getf state :playback-mode)
            (chiptune-next-playback-mode (getf state :playback-mode)))
      (dirty))
    (let ((volume (getf runtime :volume)))
      (when (and volume (or (member :volume-up commands)
                            (member :volume-down commands)))
        (let ((requested (chiptune-volume-step
                          volume (if (member :volume-up commands) 1 -1))))
          (when (/= requested volume)
            (chiptune-runtime-set-volume runtime requested)))))))

(defun chiptune-flush-block (state runtime pcm)
  "Queue PCM unless audio is off; keep the block pending while the queue is full."
  (cond
    ((not (and (getf runtime :audio-owned-p)
               (plusp (or (getf runtime :volume) 0))))
     (setf (getf state :pending-pcm) nil)
     t)
    ((write-chiptune-audio pcm)
     (setf (getf state :pending-pcm) nil)
     t)
    (t
     (setf (getf state :pending-pcm) pcm)
     nil)))

(defun chiptune-runtime-generate (state runtime now)
  "Decode and queue one 44.1 kHz block per 60 Hz tick; drive end-of-track policy."
  (let ((pending (getf state :pending-pcm)))
    (cond
      ((or (null (getf state :metadata)) (getf state :paused)) nil)
      (pending
       (chiptune-flush-block state runtime pending)
       nil)
      ((>= now (getf runtime :next-block-at))
       (multiple-value-bind (visual ended frames position pcm)
           (step-chiptune-file)
         (declare (ignore frames))
         (cond
           ((null visual)
            (chiptune-playback-failed state "native playback step failed")
            (setf (getf runtime :dirty) t)
            nil)
           (t
            (setf (getf state :visual) visual
                  (getf state :position) position)
            (chiptune-flush-block state runtime pcm)
            (when ended (chiptune-state-advance state))
            (incf (getf runtime :chiptune-blocks))
            (let ((target (+ (getf runtime :next-block-at)
                             +chiptune-block-nanoseconds+)))
              (setf (getf runtime :next-block-at)
                    (if (> now target) now target)))
            t)))))))

(defun chiptune-render-plist (state runtime)
  (let ((metadata (getf state :metadata)))
    (append (list :ready (not (null metadata))
                  :paused (getf state :paused)
                  :playback-mode (getf state :playback-mode)
                  :volume (or (getf runtime :volume) 0)
                  :status (getf state :status)
                  :position (getf state :position)
                  :visual (getf state :visual)
                  :file-index (getf state :file-index)
                  :file-count (length (getf state :files)))
            (when metadata
              (list :title (getf metadata :title)
                    :subtitle (getf metadata :subtitle)
                    :system (getf metadata :system)
                    :length (getf metadata :length)
                    :track-index (getf metadata :track-index)
                    :track-count (getf metadata :track-count))))))

(defun chiptune-runtime-random-seed (runtime files)
  (logand #xffffffff
          (logxor #x9e3779b9 (dashboard-runtime-read-clock runtime)
                  (length files))))

(defun chiptune-runtime-present (runtime)
  (ten-seconds-runtime-present runtime))

(defun chiptune-runtime-shutdown (runtime)
  (unwind-protect
      (unwind-protect
          (dashboard-runtime-shutdown runtime #'close-chiptune-audio)
        (close-chiptune-file))
    (setf (getf runtime :dirty) nil))
  runtime)

(defun chiptune-runtime-initialize (runtime)
  (check-type runtime list)
  (when (getf runtime :initialized-p)
    (error "Chiptune runtime is already initialized"))
  (let ((completed nil))
    (with-ten-seconds-cleanup
        ((unless completed (chiptune-runtime-shutdown runtime)))
      (unless (getf runtime :wayland)
        (unless (open-fbdev)
          (error "Chiptune presentation did not open"))
        (setf (getf runtime :presentation-owned-p) t))
      (unless (open-evdev-touch)
        (error "Chiptune touchscreen did not open"))
      (setf (getf runtime :touch-owned-p) t
            (getf runtime :controls-owned-p) t)
      (multiple-value-bind (gamepads error) (scan-evdev-gamepads)
        (when error
          (format *error-output*
                  "chiptune-deck: controller input unavailable: ~A~%" error))
        (format *error-output*
                "chiptune-deck: ~D THEGamepad controller(s) ready~%" gamepads)
        (finish-output *error-output*))
      (let* ((directory (getf runtime :chiptune-directory))
             (files (scan-chiptune-files directory))
             (volume (chiptune-runtime-volume)))
        (format *error-output*
                "chiptune-deck: found ~D supported file(s) in ~A~%"
                (length files) directory)
        (finish-output *error-output*)
        (setf (getf runtime :volume) volume)
        (if (open-chiptune-audio volume)
            (setf (getf runtime :audio-owned-p) t)
            (progn
              (format *error-output*
                      "chiptune-deck: cannot open audio; continuing muted~%")
              (finish-output *error-output*)))
        (let ((state (make-chiptune-player-state
                      files
                      :random-seed (chiptune-runtime-random-seed runtime files)
                      :status (concatenate 'string "ADD MUSIC TO " directory))))
          (when (and files
                     (not (chiptune-open-any
                           state (chiptune-file-candidates
                                  (length files) (1- (length files)) 1))))
            (setf (getf state :status) "CANNOT PLAY FILES")
            (format *error-output*
                    "chiptune-deck: cannot play any scanned file~%")
            (finish-output *error-output*))
          (setf (getf runtime :next-block-at)
                (dashboard-runtime-read-clock runtime)
                (getf runtime :chiptune-blocks) 0
                (getf runtime :dirty) t
                (getf runtime :initialized-p) t
                (getf runtime :running) t
                completed t)
          state)))))

(defun chiptune-runtime-run-iteration (state runtime)
  (unless (and (getf runtime :initialized-p) (getf runtime :running))
    (error "Chiptune runtime is not running"))
  (when (= (process-shutdown-p) 1)
    (setf (getf runtime :running) nil)
    (return-from chiptune-runtime-run-iteration
      (values state runtime '((:shutdown)))))
  (let ((trace nil))
    (flet ((record (item) (push item trace)))
      (let* ((now (dashboard-runtime-read-clock runtime))
             (decoded (chiptune-runtime-generate state runtime now)))
        (record (list :generate :now now :decoded (and decoded t)))
        (when (or (getf runtime :dirty) decoded)
          (unless (render-chiptune (chiptune-render-plist state runtime))
            (error "Chiptune render failed"))
          (record '(:render))
          (unless (chiptune-runtime-present runtime)
            (error "Chiptune presentation failed"))
          (record '(:present))
          (setf (getf runtime :dirty) nil))
        (let* ((poll (or (poll-native-input nil 8)
                         (error "Chiptune native input poll failed")))
               (control-count (getf poll :control-count))
               (touch-count (getf poll :touch-count))
               (commands nil))
          (record (list :poll :controls control-count :touches touch-count))
          (dotimes (index control-count)
            (let ((report (or (next-evdev-control)
                              (error "Chiptune control queue ended early"))))
              (dolist (command (chiptune-control-commands report))
                (pushnew command commands))))
          (loop repeat touch-count
                for report = (or (next-evdev-touch)
                                 (error "Chiptune touch queue ended early"))
                do (when (fourth report)
                     (let ((action (chiptune-touch-action
                                    (first report) (second report))))
                       (when action
                         (pushnew (chiptune-touch-command action) commands)))))
          (cond
            ((getf poll :touch-lost-p)
             (error "Chiptune touchscreen disconnected"))
            ((getf poll :shutdown-p)
             (setf (getf runtime :running) nil)
             (record '(:shutdown)))
            (commands
             (setf commands (nreverse commands))
             (record (list :commands commands))
             (chiptune-apply-commands state runtime commands))))
        (values state runtime (nreverse trace))))))

(defun chiptune-candidate-rehearse
    (runtime &key (iteration-limit 1) stop-predicate)
  "Run an opt-in bounded chiptune candidate and return iteration traces."
  (check-type iteration-limit (integer 0 *))
  (when stop-predicate (check-type stop-predicate function))
  (when (getf runtime :initialized-p)
    (error "Chiptune runtime is already initialized"))
  (let ((current nil) (iteration 0) (traces nil) (reason nil) (owned nil))
    (with-ten-seconds-cleanup ((when owned (chiptune-runtime-shutdown runtime)))
      (setf current (chiptune-runtime-initialize runtime) owned t)
      (loop
        (cond
          ((not (getf runtime :running)) (setf reason :shutdown) (return))
          ((>= iteration iteration-limit) (setf reason :limit) (return))
          ((and stop-predicate
                (funcall stop-predicate current runtime iteration))
           (setf reason :operator-stop)
           (return)))
        (multiple-value-bind (next ignored-runtime trace)
            (chiptune-runtime-run-iteration current runtime)
          (declare (ignore ignored-runtime))
          (setf current next)
          (push trace traces)
          (incf iteration)))
      (values current runtime (nreverse traces) reason))))

(defun run-chiptune-main (&optional arguments)
  (unless (and (= (length arguments) 1) (stringp (first arguments)))
    (format *error-output* "usage: chiptune-deck CHIPTUNE_DIRECTORY~%")
    (finish-output *error-output*)
    (return-from run-chiptune-main 2))
  (let ((runtime (make-chiptune-runtime :directory (first arguments))))
    (let ((state (chiptune-runtime-initialize runtime)))
      (unwind-protect
          (loop while (getf runtime :running)
                do (setf state (chiptune-runtime-run-iteration state runtime)))
        (chiptune-runtime-shutdown runtime))
      0)))
