(in-package #:retrodeck)

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
