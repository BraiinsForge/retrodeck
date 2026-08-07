(in-package #:retrodeck)

(defparameter *dashboard-systems*
  '((:nes "NES")
    (:gb "GAME BOY")
    (:gbc "GBC")
    (:gba "GBA")
    (:zx "ZX SPECTRUM")
    (:deck "DECK")))

(defparameter *dashboard-palette*
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
    (:muted . #x949494)))

(defparameter *dashboard-executables*
  '((:nes . "/mnt/data/nes-deck/nes-deck")
    (:gb . "/mnt/data/nes-deck/gb-deck")
    (:gba . "/mnt/data/nes-deck/gba-deck")
    (:zx . "/mnt/data/nes-deck/zx-deck")
    (:deck . "/mnt/data/nes-deck/ten-seconds-deck")
    (:doom . "/mnt/data/nes-deck/doom-deck")
    (:chiptunes . "/mnt/data/nes-deck/chiptune-deck")
    (:node-mode . "/mnt/data/nes-deck/menu/node-mode")
    (:terminal . "/mnt/data/nes-deck/terminal/retro-terminal")
    (:reboot . "/sbin/reboot")))

(defparameter *dashboard-doom-state-directory* "/mnt/data/doom")
(defparameter *dashboard-cover-directory* "/mnt/data/nes-deck/covers/")
(defparameter *dashboard-settings-icon-path*
  "/mnt/data/nes-deck/menu/settings-icon.png")

(defparameter *dashboard-built-in-applications*
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
    (:id "node-mode"
     :title "NODE MODE"
     :system :deck
     :rom "/mnt/data/nes-deck/menu/node-mode"
     :color #xfe6c27)
    (:id "reboot"
     :title "REBOOT"
     :system :deck
     :rom "/sbin/reboot"
     :color #xd75f5f)))

(defun copy-dashboard-policy-value (value)
  (typecase value
    (cons (cons (copy-dashboard-policy-value (car value))
                (copy-dashboard-policy-value (cdr value))))
    (string (copy-seq value))
    (t value)))

(defun dashboard-tsv-lines (text)
  (loop with start = 0
        while (< start (length text))
        for end = (position #\Newline text :start start)
        collect (subseq text start (or end (length text)))
        do (if end (setf start (1+ end)) (loop-finish))))

(defun dashboard-tsv-fields (line)
  (loop with start = 0
        for end = (position #\Tab line :start start)
        collect (subseq line start (or end (length line)))
        do (if end (setf start (1+ end)) (loop-finish))))

(defun dashboard-bootstrap-path (path description)
  (let ((name (namestring (pathname path))))
    (unless (and (plusp (length name)) (char= (char name 0) #\/))
      (error "~A path must be absolute" description))
    name))

(defun dashboard-manifest-id-p (value)
  (and (<= 1 (length value) 48)
       (loop for character across value
             for position from 0
             always
             (or (and (char<= #\a character) (char<= character #\z))
                 (digit-char-p character)
                 (and (char= character #\-)
                      (plusp position)
                      (< position (1- (length value)))
                      (not (char= (char value (1- position)) #\-)))))))

(defun dashboard-manifest-system (name line-number)
  (or (cdr (assoc name '(("nes" . :nes) ("gb" . :gb) ("gbc" . :gbc)
                         ("gba" . :gba) ("zx" . :zx) ("deck" . :deck))
                  :test #'string=))
      (error "Invalid system on manifest line ~D" line-number)))

(defun dashboard-rgb-color (text context)
  (unless (and (= (length text) 7) (char= (char text 0) #\#)
               (loop for index from 1 below (length text)
                     always (digit-char-p (char text index) 16)))
    (error "Invalid #RRGGBB color in ~A" context))
  (parse-integer text :start 1 :radix 16))

(defun dashboard-manifest-header-p (fields)
  (and (= (length fields) 5)
       (equal (subseq fields 0 4) '("id" "title" "system" "rom"))
       (member (fifth fields) '("color" "#RRGGBB") :test #'string=)))

(defun load-dashboard-games (path)
  "Read a launcher-selected effective manifest after its native validation gate."
  (let* ((name (dashboard-bootstrap-path path "Manifest"))
         (contents (read-bounded-regular-file name 1 65536)))
    (unless (stringp contents)
      (error "Cannot read dashboard manifest ~A" name))
    (let ((ids (make-hash-table :test #'equal))
          (roms (make-hash-table :test #'equal))
          (games nil)
          (saw-data nil))
      (loop for raw-line in (dashboard-tsv-lines contents)
            for line-number from 1
            for line = (if (and (plusp (length raw-line))
                                (char= (char raw-line (1- (length raw-line)))
                                       #\Return))
                           (subseq raw-line 0 (1- (length raw-line)))
                           raw-line)
            do (when (> (length raw-line) 4096)
                 (error "Manifest line ~D exceeds 4096 bytes" line-number))
            unless (or (zerop (length line)) (char= (char line 0) #\#))
              do (let ((fields (dashboard-tsv-fields line)))
                (cond
                  ((and (not saw-data) (dashboard-manifest-header-p fields))
                   (setf saw-data t))
                  (t
                   (setf saw-data t)
                   (unless (= (length fields) 5)
                     (error "Manifest line ~D must have 5 fields" line-number))
                   (destructuring-bind (id raw-title system-name rom color) fields
                     (unless (dashboard-manifest-id-p id)
                       (error "Invalid id on manifest line ~D" line-number))
                     (let ((title
                             (display-utf8-bytes-ascii raw-title 64)))
                       (unless (and (plusp (length title))
                                    (not (char= (char raw-title 0) #\Space))
                                    (not (char= (char raw-title
                                                      (1- (length raw-title)))
                                                #\Space)))
                         (error "Invalid title on manifest line ~D" line-number))
                       (unless (and (<= 1 (length rom) 4095)
                                    (char= (char rom 0) #\/)
                                    (not (char= (char rom (1- (length rom)))
                                                #\Space)))
                         (error "Invalid ROM path on manifest line ~D" line-number))
                       (display-utf8-bytes-ascii rom 4095)
                       (when (gethash id ids)
                         (error "Duplicate id on manifest line ~D" line-number))
                       (when (gethash rom roms)
                         (error "Duplicate ROM on manifest line ~D" line-number))
                       (setf (gethash id ids) t
                             (gethash rom roms) t)
                       (push (list :id id :title title
                                   :system (dashboard-manifest-system
                                            system-name line-number)
                                   :rom rom
                                   :color (dashboard-rgb-color
                                           color
                                           (format nil "manifest line ~D"
                                                   line-number)))
                             games)
                       (when (> (length games) 64)
                         (error "Manifest contains more than 64 games"))))))))
      (unless games
        (error "Manifest contains no games"))
      (nreverse games))))

(defun dashboard-palette-role (name)
  (car (find name *dashboard-palette* :test #'string=
             :key (lambda (entry)
                    (string-downcase (symbol-name (car entry)))))))

(defun dashboard-palette-icon-p (name)
  (and (<= 1 (length name) 64)
       (loop for character across name
             always (or (and (char<= #\a character)
                             (char<= character #\z))
                        (digit-char-p character)
                        (char= character #\-)))))

(defun load-dashboard-palette (path)
  "Read one complete dashboard palette without changing startup policy."
  (let* ((name (dashboard-bootstrap-path path "Palette"))
         (contents (read-bounded-regular-file name 1 4096)))
    (unless (stringp contents)
      (error "Cannot read dashboard palette ~A" name))
    (let ((colors (make-hash-table :test #'eq))
          (saw-settings-icon nil))
      (loop for raw-line in (dashboard-tsv-lines contents)
            for line-number from 1
            for line = (if (and (plusp (length raw-line))
                                (char= (char raw-line (1- (length raw-line)))
                                       #\Return))
                           (subseq raw-line 0 (1- (length raw-line)))
                           raw-line)
            unless (or (zerop (length line)) (char= (char line 0) #\#))
              do (let ((fields (dashboard-tsv-fields line)))
                   (unless (= (length fields) 2)
                     (error "Palette line ~D must have 2 fields" line-number))
                   (if (string= (first fields) "settings-icon")
                       (progn
                         (when (or saw-settings-icon
                                   (not (dashboard-palette-icon-p
                                         (second fields))))
                           (error "Invalid legacy settings icon on palette line ~D"
                                  line-number))
                         (setf saw-settings-icon t))
                       (let ((role (dashboard-palette-role (first fields))))
                         (unless role
                           (error "Unknown palette role on line ~D" line-number))
                         (when (nth-value 1 (gethash role colors))
                           (error "Duplicate palette role on line ~D" line-number))
                         (setf (gethash role colors)
                               (dashboard-rgb-color
                                (second fields)
                                (format nil "palette line ~D" line-number)))))))
      (loop for entry in *dashboard-palette*
            for role = (car entry)
            collect
            (multiple-value-bind (color present-p) (gethash role colors)
              (unless present-p
                (error "Palette is missing role ~(~A~)" role))
              (cons role color))))))

(defun load-dashboard-bootstrap (manifest-path palette-path)
  "Return rehearsal policy data without installing it as dashboard authority."
  (let ((games (append (load-dashboard-games manifest-path)
                       (copy-dashboard-policy-value
                        *dashboard-built-in-applications*)))
        (fallback (copy-dashboard-policy-value *dashboard-palette*)))
    (handler-case
        (values games (load-dashboard-palette palette-path) t)
      (error (condition)
        (format *error-output*
                "retrodeck: ~A; using startup dashboard palette~%" condition)
        (finish-output *error-output*)
        (values games fallback nil)))))

(defparameter *dashboard-timings*
  '((:child-touch-exit-ms . 2000)
    (:child-term-grace-ms . 4000)
    (:reboot-confirm-ms . 4000)
    (:controller-burst-window-ms . 1000)
    (:controller-quiet-reset-ms . 1000)
    (:controller-scan-ms . 1000)
    (:touch-reconnect-ms . 1000)
    (:main-poll-ms . 250)
    (:animated-poll-ms . 66)
    (:network-refresh-ms . 2000)
    (:console-mirror-ms . 100)))

(defparameter *dashboard-volume-default* 42)
(defparameter *dashboard-volume-step* 5)
(defparameter *dashboard-brightness-minimum* 10)
(defparameter *dashboard-brightness-step* 10)
(defparameter *dashboard-controller-burst-limit* 12)

(defparameter *dashboard-keyboard-controls*
  '((1 :back)
    (15 :system-next :system-previous)
    (28 :confirm)
    (96 :confirm)
    (103 :up)
    (105 :left)
    (106 :right)
    (108 :down)))

(defparameter *dashboard-gamepad-controls*
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
    (#x800 . :down)))

(defun dashboard-control-actions (report)
  (check-type report list)
  (case (getf report :kind)
    (:keyboard
     (let* ((definition (assoc (getf report :code)
                               *dashboard-keyboard-controls* :test #'=))
            (action (and definition
                         (if (and (getf report :shift) (third definition))
                             (third definition)
                             (second definition)))))
       (and action (list action))))
    (:gamepad
     (let ((edges (getf report :edges)))
       (check-type edges (integer 1 4095))
       (remove-duplicates
        (loop for (mask . action) in *dashboard-gamepad-controls*
              when (logtest mask edges)
                collect action)
        :test #'eq)))
    (otherwise
     (error "Unknown dashboard control report ~S" report))))

(defun collect-dashboard-control-actions ()
  (let ((gamepad nil)
        (keyboard nil)
        (report-count 0))
    (loop for report = (next-evdev-control)
          while report
          do (incf report-count)
             (dolist (action (dashboard-control-actions report))
               (if (eq (getf report :kind) :gamepad)
                   (pushnew action gamepad :test #'eq)
                   (pushnew action keyboard :test #'eq))))
    (values gamepad keyboard report-count)))

(defun dashboard-controller-guard-initial-state ()
  (list :edge-times nil :suspended nil :last-edge-at nil))

(defun dashboard-controller-guard-accept-edge (state now)
  (check-type state list)
  (check-type now (integer 0 *))
  (let ((next (copy-list state)))
    (setf (getf next :last-edge-at) now)
    (when (getf next :suspended)
      (return-from dashboard-controller-guard-accept-edge
        (values next nil nil)))
    (let* ((window (dashboard-timing :controller-burst-window-ms))
           (edge-times
             (remove-if (lambda (time) (>= (- now time) window))
                        (getf next :edge-times)))
           (updated (append edge-times (list now))))
      (setf (getf next :edge-times) updated)
      (if (<= (length updated) *dashboard-controller-burst-limit*)
          (values next t nil)
          (progn
            (setf (getf next :suspended) t)
            (values next nil t))))))

(defun dashboard-controller-guard-recover-if-quiet (state now)
  (check-type state list)
  (check-type now (integer 0 *))
  (let ((last-edge-at (getf state :last-edge-at)))
    (if (and (getf state :suspended)
             (integerp last-edge-at)
             (>= (- now last-edge-at)
                 (dashboard-timing :controller-quiet-reset-ms)))
        (values (dashboard-controller-guard-initial-state) t)
        (values (copy-list state) nil))))

(defun dashboard-controller-scan-due-p (last-scan-ms now
                                         &key force rescan)
  (check-type last-scan-ms (or null (integer 0 *)))
  (check-type now (integer 0 *))
  (or force rescan (null last-scan-ms) (zerop last-scan-ms)
      (>= (- now last-scan-ms) (dashboard-timing :controller-scan-ms))))

(defun dashboard-controller-input-actions
    (gamepad-actions keyboard-actions guard now
     &key (controller-quarantined-p
            (menu-sound-blocks-input-p :controller now)))
  (check-type gamepad-actions list)
  (check-type keyboard-actions list)
  (let ((next-guard guard)
        (accepted-gamepad gamepad-actions)
        (newly-suspended nil))
    (when gamepad-actions
      (multiple-value-bind (updated accepted suspended)
          (dashboard-controller-guard-accept-edge guard now)
        (setf next-guard updated
              newly-suspended suspended)
        (unless accepted
          (setf accepted-gamepad nil))))
    (when controller-quarantined-p
      (setf accepted-gamepad nil))
    (values (remove-duplicates
             (append accepted-gamepad keyboard-actions) :test #'eq)
            next-guard newly-suspended)))

(defun dashboard-controller-command (actions modal-view settings-view)
  (check-type actions list)
  (cond
    ((and (or modal-view settings-view) (member :back actions)) :back)
    (modal-view nil)
    ((member :settings actions) :settings)
    ((and (not settings-view) (member :system-previous actions))
     :system-previous)
    ((and (not settings-view) (member :system-next actions)) :system-next)
    ((or (member :left actions) (member :up actions)) :previous)
    ((or (member :right actions) (member :down actions)) :next)
    ((member :confirm actions) :confirm)
    (t nil)))

(defparameter *dashboard-reboot-confirmation-text*
  "PRESS A OR TAP AGAIN TO REBOOT")
(defparameter *dashboard-loop-labels*
  '(:touch-waiting "WAITING FOR TOUCHSCREEN"
    :touch-reconnected "TOUCHSCREEN RECONNECTED"
    :rebooting "REBOOTING"
    :reboot-error "REBOOT ERROR - CHECK LOG"
    :reboot-not-started "REBOOT DID NOT START"
    :reboot-cancelled "REBOOT CANCELLED"
    :reboot-exited "REBOOT COMMAND EXITED"
    :game-error "GAME ERROR - CHECK LOG"
    :game-not-started "GAME DID NOT START"))
(defparameter *dashboard-terminal-login-shell* "/BIN/ASH")
(defparameter *dashboard-reduced-motion-environment*
  "RETRO_DECK_REDUCED_MOTION")
(defparameter *dashboard-wayland-display-environment* "WAYLAND_DISPLAY")

(defun dashboard-system-label (system)
  (let* ((name (etypecase system
                 (string system)
                 (symbol (string-downcase (symbol-name system)))))
         (definition
           (find name *dashboard-systems*
                 :key (lambda (entry)
                        (string-downcase (symbol-name (first entry))))
                 :test #'string=)))
    (if definition
        (second definition)
        (map 'string
             (lambda (character)
               (if (< (char-code character) 128) character #\?))
             name))))

(defun dashboard-color (role)
  (let ((entry (assoc role *dashboard-palette* :test #'eq)))
    (if entry
        (cdr entry)
        (error "Unknown dashboard color role ~S" role))))

(defun dashboard-loop-label (name)
  (let ((value (getf *dashboard-loop-labels* name)))
    (if value
        value
        (error "Unknown dashboard loop label ~S" name))))

(defun dashboard-timing (name)
  (let ((entry (assoc name *dashboard-timings* :test #'eq)))
    (if entry
        (cdr entry)
        (error "Unknown dashboard timing ~S" name))))

(defun dashboard-executable (route)
  (let ((entry (assoc route *dashboard-executables* :test #'eq)))
    (if entry
        (cdr entry)
        (error "Unknown dashboard executable route ~S" route))))

(defun dashboard-application (id)
  (let ((application
          (find id *dashboard-built-in-applications*
                :key (lambda (entry) (getf entry :id))
                :test #'string=)))
    (and application (copy-dashboard-policy-value application))))

(defun dashboard-application-id-p (application id)
  (and (eq (getf application :system) :deck)
       (string= (getf application :id) id)))

(defun dashboard-wad-application-p (application)
  "True for a Deck entry whose content is a DOOM WAD.
Routing on the extension rather than on one fixed id lets a second IWAD be
added as another catalog entry without changing any code."
  (let ((rom (getf application :rom)))
    (and (eq (getf application :system) :deck)
         (stringp rom)
         (> (length rom) 4)
         (string-equal ".wad" rom :start2 (- (length rom) 4)))))

(defun dashboard-application-route (application)
  (case (getf application :system)
    (:nes :nes)
    ((:gb :gbc) :gb)
    (:gba :gba)
    (:zx :zx)
    (:deck (cond ((dashboard-application-id-p application "node-mode")
                 :node-mode)
                ((dashboard-application-id-p application "chiptunes")
                  :chiptunes)
                 ((dashboard-wad-application-p application) :doom)
                 (t :deck)))
    (otherwise
     (error "Unknown application system ~S" (getf application :system)))))

(defun dashboard-launch-plan (application volume-percent
                              &key (keymap "us") wayland volume-state)
  (check-type volume-percent (integer 0 100))
  (check-type keymap string)
  (when volume-state
    (check-type volume-state string))
  (let ((terminal-mode (getf application :terminal-mode)))
    (cond
      (terminal-mode
       (list :executable (dashboard-executable :terminal)
             :arguments (list terminal-mode)
             :environment (list (cons "RETRO_DECK_KEYMAP" keymap))
             :label (if (string= terminal-mode "shell")
                        "terminal"
                        (format nil "~A REPL" terminal-mode))
             :touch-supervision t
             :mirror-console t))
      ((dashboard-application-id-p application "reboot")
       (list :executable (getf application :rom)
             :arguments nil
             :environment nil
             :label "reboot"
             :touch-supervision t
             :mirror-console nil))
      (t
       (let* ((system (getf application :system))
              (route (dashboard-application-route application))
              (arguments
                (if (or (not (eq system :deck))
                        (member route '(:chiptunes :doom) :test #'eq))
                    (list (getf application :rom))
                    nil))
              (environment
                (list (cons "RETRO_DECK_VOLUME_PERCENT"
                            (format nil "~D" volume-percent)))))
         ;; DOOM is a Deck entry but plays like a console: it gets the exit
         ;; cross and the two-second hold instead of 10 Seconds' own BACK.
         (when (or (not (eq system :deck)) (eq route :doom))
           (setf environment
                 (append environment
                         (list (cons "RETRO_DECK_EXIT_HINT" "1")))))
         ;; Savegames and default.cfg cannot live beside the WAD: the
         ;; installed games directory is replaced on every activation.
         (when (eq route :doom)
           (setf environment
                 (append environment
                         (list (cons "RETRO_DECK_DOOM_STATE"
                                     *dashboard-doom-state-directory*)))))
         (when wayland
           (setf environment
                 (append environment
                         (list (cons "RETRO_DECK_PRESENTATION"
                                     "widget")))))
         (when (and volume-state (plusp (length volume-state)))
           (setf environment
                 (append environment
                         (list (cons "RETRO_DECK_VOLUME_STATE"
                                     volume-state)))))
         (list :executable (dashboard-executable route)
               :arguments arguments
               :environment environment
               :label (getf application :id)
               :touch-supervision (or wayland
                                      (not (eq system :deck))
                                      (eq route :doom))
               :mirror-console nil))))))

(defun reboot-confirmation-active-p (deadline now)
  (and (plusp deadline) (< now deadline)))

(defparameter *chiptune-extensions*
  '(".ay" ".gbs" ".gym" ".hes" ".kss" ".nsf" ".nsfe" ".sap" ".spc"
    ".vgm" ".vgz" ".ogg"))
(defparameter *chiptune-maximum-files* 1024)
(defparameter *chiptune-maximum-file-size* (* 16 1024 1024))
(defparameter *chiptune-maximum-depth* 4)

(defun chiptune-file-accepted-p (path size)
  (check-type path string)
  (check-type size integer)
  (let ((dot (position #\. path :from-end t)))
    (and (<= 1 size *chiptune-maximum-file-size*) dot
         (not (null (member
                     (map 'string (lambda (character)
                                    (if (char<= #\A character #\Z)
                                        (char-downcase character) character))
                          (subseq path dot))
                     *chiptune-extensions* :test #'string=))))))

(defparameter *main-routes*
  '(("deck-menu" . :dashboard)
    ("ten-seconds-deck" . :ten-seconds)
    ("chiptune-deck" . :chiptunes)))

(defun main-invocation-name (program)
  (check-type program string)
  (let ((slash (position #\/ program :from-end t)))
    (if slash (subseq program (1+ slash)) program)))

(defun main-route (arguments)
  "Choose the app entry point from the launcher invocation name."
  (check-type arguments list)
  (let* ((program (or (first arguments) ""))
         (route (cdr (assoc (main-invocation-name program) *main-routes*
                            :test #'string=))))
    (values (or route :startup) (rest arguments))))

(defun scan-chiptune-files (directory &key (lister #'list-native-directory))
  "Collect supported chiptune paths the way the C++ player scanned them."
  (check-type directory string)
  (check-type lister function)
  (let ((files nil) (count 0))
    (labels ((entry-name (entry) (getf entry :name))
             (visible-p (entry)
               (let ((name (entry-name entry)))
                 (and (plusp (length name))
                      (char/= (char name 0) #\.))))
             (walk (path depth)
               (when (or (> depth *chiptune-maximum-depth*)
                         (>= count *chiptune-maximum-files*))
                 (return-from walk))
               (dolist (entry (sort (remove-if-not #'visible-p
                                                   (funcall lister path))
                                    #'string< :key #'entry-name))
                 (when (>= count *chiptune-maximum-files*)
                   (return))
                 (let ((child (concatenate 'string path "/"
                                           (entry-name entry))))
                   (ecase (getf entry :kind)
                     (:directory (walk child (1+ depth)))
                     (:file
                      (when (chiptune-file-accepted-p
                             child (getf entry :size))
                        (push child files)
                        (incf count)))
                     (:other nil))))))
      (walk directory 0))
    (sort (nreverse files) #'string<)))

(defun chiptune-display-text (input maximum)
  (check-type input string)
  (check-type maximum (integer 0 *))
  (let ((clean
          (with-output-to-string (output)
            (loop for character across input do
              (cond ((char<= #\a character #\z)
                     (write-char (char-upcase character) output))
                    ((or (char<= #\A character #\Z)
                         (char<= #\0 character #\9)
                         (find character " .:-+/" :test #'char=))
                     (write-char character output))
                    ((or (char= character #\_) (char= character #\Tab))
                     (write-char #\Space output)))))))
    (cond ((<= (length clean) maximum) clean)
          ((<= maximum 3) (subseq clean 0 maximum))
          (t (concatenate 'string (subseq clean 0 (- maximum 3)) "...")))))

(defun chiptune-base-name (path)
  (check-type path string)
  (let* ((slash (position #\/ path :from-end t))
         (name (subseq path (if slash (1+ slash) 0)))
         (dot (position #\. name :from-end t))
         (stem (subseq name 0 (or dot (length name)))))
    (substitute #\Space #\_ (substitute #\Space #\- stem))))

(defun chiptune-format-time (milliseconds)
  (check-type milliseconds integer)
  (let ((seconds (max 0 (truncate milliseconds 1000))))
    (format nil "~D:~2,'0D" (truncate seconds 60) (mod seconds 60))))
