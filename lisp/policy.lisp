(in-package #:retrodeck)

(defparameter *dashboard-systems*
  '((:nes "NES")
    (:gb "GAME BOY")
    (:gbc "GBC")
    (:zx "ZX SPECTRUM")
    (:chip8 "CHIP-8")
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
    (:zx . "/mnt/data/nes-deck/zx-deck")
    (:chip8 . "/mnt/data/nes-deck/chip8-deck")
    (:deck . "/mnt/data/nes-deck/ten-seconds-deck")
    (:chiptunes . "/mnt/data/nes-deck/chiptune-deck")
    (:terminal . "/mnt/data/nes-deck/terminal/retro-terminal")
    (:reboot . "/sbin/reboot")))

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

(defparameter *uploader-policy*
  '(:paths
    (:password-config "/mnt/data/nes-deck/uploader/password.conf"
     :address-config "/mnt/data/nes-deck/uploader/address.conf"
     :rom-root "/mnt/data/roms"
     :base-catalog "/mnt/data/nes-deck/menu/games.tsv"
     :upload-catalog "/mnt/data/nes-deck/uploads/games.tsv"
     :active-palette "/mnt/data/nes-deck/state/palette.tsv"
     :base-palette "/mnt/data/nes-deck/menu/palette.tsv"
     :palette-override "/mnt/data/nes-deck/state/dashboard-palette.sexp")
    :http
    (:address "0.0.0.0:8080" :port "8080"
     :required-host-address-family :ipv4-or-mapped-ipv6
     :required-host-port "8080"
     :origin-allow-missing t :origin-allow-null t
     :origin-match :exact-http-request-host
     :routes ((:get "/") (:post "/login") (:post "/logout")
              (:post "/upload") (:post "/palette")
              (:get "/assets/paper.css") (:get "/assets/palette.js"))
     :read-header-timeout-ms 5000 :read-timeout-ms 35000
     :write-timeout-ms 35000 :idle-timeout-ms 30000
     :maximum-address-config-bytes 64 :maximum-header-bytes 16384
     :maximum-login-bytes 512 :maximum-palette-bytes 4096
     :multipart-memory-bytes 1048576 :maximum-request-bytes 12582912
     :urlencoded-content-type "application/x-www-form-urlencoded"
     :urlencoded-routes ("/login" "/palette")
     :form-fields
     (:login ("password") :logout ("csrf")
      :upload ("csrf" "system" "title" "rom")
      :palette ("csrf" :palette-fields))
     :upload-file-field "rom" :maximum-upload-files 1
     :response-headers
     (("Cache-Control" . "no-store")
      ("Content-Security-Policy" .
       "default-src 'none'; img-src 'self'; style-src 'self'; script-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'")
      ("Cross-Origin-Opener-Policy" . "same-origin")
      ("Cross-Origin-Resource-Policy" . "same-origin")
      ("Referrer-Policy" . "no-referrer")
      ("X-Content-Type-Options" . "nosniff")
      ("X-Frame-Options" . "DENY")))
    :authentication
    (:password-config-version 1 :password-derivation :pbkdf2-hmac-sha256
     :password-iterations 210000
     :minimum-password-bytes 8 :maximum-password-bytes 128
     :password-config-private-regular t
     :password-config-root-owned-when-root t
     :minimum-config-iterations 100000
     :maximum-config-iterations 1000000 :maximum-config-bytes 1024
     :salt-bytes 16 :digest-bytes 32 :token-bytes 32
     :cookie-token-characters 43 :csrf-token-bytes 32
     :password-config-fields ("version" "iterations" "salt" "digest")
     :session-cookie "deck_rom_session" :session-cookie-path "/"
     :session-cookie-http-only t :session-cookie-same-site :strict
     :session-cookie-max-age-seconds 28800
     :session-cookie-delete-max-age -1 :session-lifetime-ms 28800000
     :session-storage :memory-only :session-key-digest :sha256
     :session-eviction :earliest-expiry
     :maximum-sessions 8 :maximum-login-sources 256
     :login-source-eviction :least-recently-seen
     :login-gate-capacity 1 :failures-before-lock 5
     :lock-duration-ms 300000 :login-check-wait-ms 2000
     :busy-retry-after-seconds 3)
    :dashboard
    (:restart-executable "/etc/init.d/nes-deck" :restart-arguments ("restart")
     :restart-timeout-ms 20000)
    :storage
    (:rom-directory-mode #o755 :private-directory-mode #o700
     :private-file-mode #o600)
    :rom
    (:minimum-title-runes 1 :maximum-title-runes 64
     :title-must-equal-trimmed t :title-requires-valid-utf8 t
     :title-forbid-controls t
     :minimum-rom-bytes 1 :minimum-slug-bytes 1 :maximum-slug-bytes 32
     :slug-case-fold :unicode-lowercase :slug-alphabet "abcdefghijklmnopqrstuvwxyz0123456789"
     :slug-separator "-" :slug-collapse-separators t
     :slug-trim-separators t
     :identifier-components ("upload" :system :slug)
     :maximum-identifier-bytes 48 :identifier-trim-separators t
     :duplicate-catalog-keys (:identifier :rom-path)
     :upload-sort (:system :title)
     :archive-extension ".zip" :accepted-upload-kinds (:raw :single-rom-zip)
     :raw-extension :system-extension :extension-match :case-insensitive
     :maximum-archive-bytes 10485760 :archive-nondirectory-members 1
     :archive-reject-encrypted t :archive-reject-symlink t
     :archive-member-basename-only t :archive-reject-backslash t
     :archive-member-extension :system-extension
     :archive-uncompressed-limit :system-maximum
     :archive-revalidate-rom t
     :maximum-catalog-bytes 65536 :catalog-file-kind :regular-no-symlink
     :catalog-scanner-maximum-bytes 4097 :catalog-field-count 5
     :catalog-fields ("id" "title" "system" "rom" "color")
     :catalog-ignore (:blank :hash-comment) :catalog-accept-crlf t
     :base-catalog-missing :error :upload-catalog-missing :empty
     :maximum-games 64 :maximum-games-scope :base-plus-upload
     :persistence-order (:install-rom :write-upload-catalog :restart-dashboard)
     :catalog-write-failure
     (:remove-installed-rom :best-effort :catalog-state :possibly-renamed)
     :restart-failure :retain-rom-and-catalog-and-report)
    :palette
    (:minimum-bytes 1 :maximum-bytes 4096
     :canonical-rgb-characters 7 :field-count 22
     :legacy-tsv-field "settings-icon"
     :override-version-field ":version"
     :override-legacy-icon-field ":settings-icon"
     :override-palette-field ":palette"
     :minimum-legacy-icon-name-bytes 1
     :maximum-legacy-icon-name-bytes 64 :maximum-override-token-bytes 64
     :maximum-override-tokens 54 :accepted-override-versions (2 3)
     :written-override-version 2
     :base-read-order (:base-palette :active-palette)
     :base-read-policy :first-valid
     :override-read-policy :apply-only-valid-ignore-missing-or-invalid
     :persistence-order (:write-override :restart-dashboard)
     :write-failure-state :possibly-renamed-no-restart
     :restart-failure :retain-override-and-report)
    :systems
    ((:id "nes" :label "NES" :extension ".nes" :color "#FF5F00"
      :maximum-rom-bytes 8388608
      :validation (:minimum-header-bytes 16 :magic-hex "4E45531A"))
     (:id "gb" :label "Game Boy" :extension ".gb" :color "#87AF87"
      :maximum-rom-bytes 8388608
      :validation (:minimum-header-bytes #x150 :logo :nintendo
                   :logo-offset #x104 :logo-bytes 48
                   :checksum :game-boy-header-complement
                   :checksum-range (#x134 #x14c) :checksum-byte #x14d))
     (:id "gbc" :label "Game Boy Color" :extension ".gbc" :color "#5F87D7"
      :maximum-rom-bytes 8388608
      :validation (:minimum-header-bytes #x150 :logo :nintendo
                   :logo-offset #x104 :logo-bytes 48
                   :checksum :game-boy-header-complement
                   :checksum-range (#x134 #x14c) :checksum-byte #x14d
                   :color-flag-byte #x143 :color-flags (#x80 #xc0)))
     (:id "zx" :label "ZX Spectrum" :extension ".tap" :color "#AF87D7"
      :maximum-rom-bytes 8388608
      :validation (:minimum-bytes 4 :block-length-endian :little
                   :minimum-block-bytes 2 :checksum :xor-zero
                   :minimum-blocks 1 :exact-framing t))
     (:id "chip8" :label "CHIP-8" :extension ".ch8" :color "#5FD7D7"
      :maximum-rom-bytes 65024 :validation :size-only))
    :palette-fields
    (("background" . "Background") ("text-dark" . "Dark text")
     ("field" . "Field") ("surface" . "Surface")
     ("inactive-border" . "Inactive border")
     ("control-border" . "Control border") ("footer" . "Footer")
     ("inactive-text" . "Inactive text") ("text" . "Text")
     ("white" . "Bright white") ("title" . "Title")
     ("volume-off" . "Volume off") ("volume-on" . "Volume on")
     ("selected" . "Selected item") ("wifi-active" . "Wi-Fi active")
     ("wifi-focus" . "Wi-Fi focus")
     ("wifi-active-border" . "Wi-Fi active border")
     ("field-label" . "Field label") ("accent" . "Accent")
     ("active" . "Active control")
     ("control-surface" . "Control surface")
     ("muted" . "Muted control"))))

(defun uploader-policy-section (section)
  (check-type section keyword)
  (let ((value (getf *uploader-policy* section :missing)))
    (when (eq value :missing)
      (error "Unknown uploader policy section ~S" section))
    (copy-dashboard-policy-value value)))

(defun uploader-policy-snapshot ()
  (copy-dashboard-policy-value *uploader-policy*))

(defun uploader-system-policy (id)
  (check-type id string)
  (let ((system (find id (getf *uploader-policy* :systems)
                      :key (lambda (entry) (getf entry :id))
                      :test #'string=)))
    (and system (copy-dashboard-policy-value system))))

(defun encode-uploader-palette-override (values)
  (with-output-to-string (output)
    (format output "(:version 2~% :palette~%  (")
    (loop for (name . color) in values
          for first = t then nil
          do (unless first (format output "~%   "))
             (format output ":~A \"~A\"" name color))
    (format output "))~%")))

(defun uploader-palette-error (status message)
  (list :accepted-p nil :status status :error (copy-seq message)))

(defun uploader-palette-save-plan (fields)
  "Validate one decoded palette form and return its non-authoritative save plan."
  (check-type fields list)
  (let ((specs (getf *uploader-policy* :palette-fields))
        (values (make-hash-table :test #'equal)))
    (dolist (field fields)
      (unless (and (consp field) (stringp (car field)) (stringp (cdr field))
                   (assoc (car field) specs :test #'string=)
                   (not (nth-value 1 (gethash (car field) values))))
        (return-from uploader-palette-save-plan
          (uploader-palette-error
           400 "The appearance form contains an unknown or repeated field.")))
      (handler-case
          (setf (gethash (car field) values)
                (format nil "#~6,'0X"
                        (dashboard-rgb-color (cdr field) "appearance form")))
        (error ()
          (return-from uploader-palette-save-plan
            (uploader-palette-error
             400 "Every color must be a full RGB value like #12ABEF.")))))
    (unless (= (hash-table-count values) (length specs))
      (return-from uploader-palette-save-plan
        (uploader-palette-error
         422 "palette must contain every dashboard color exactly once")))
    (let* ((ordered (loop for spec in specs
                          for name = (car spec)
                          collect (cons name (gethash name values))))
           (paths (getf *uploader-policy* :paths))
           (dashboard (getf *uploader-policy* :dashboard)))
      (copy-dashboard-policy-value
       (list :accepted-p t :status 200
             :notice "Dashboard appearance was saved and applied."
             :failure-status 422
             :write (list :path (getf paths :palette-override)
                          :mode (getf (getf *uploader-policy* :storage)
                                     :private-file-mode)
                          :contents (encode-uploader-palette-override ordered)
                          :error-prefix "save dashboard appearance: ")
             :restart (list :executable (getf dashboard :restart-executable)
                            :arguments (getf dashboard :restart-arguments)
                            :timeout-ms (getf dashboard :restart-timeout-ms)
                            :error-prefix
                            "appearance was saved, but the dashboard could not reload: "))))))

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
                         ("zx" . :zx) ("chip8" . :chip8) ("deck" . :deck))
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
    (:animated-poll-ms . 40)
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

(defun dashboard-application-route (application)
  (case (getf application :system)
    (:nes :nes)
    ((:gb :gbc) :gb)
    (:zx :zx)
    (:deck (if (dashboard-application-id-p application "chiptunes")
               :chiptunes
               :deck))
    (otherwise :chip8)))

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
                        (eq route :chiptunes))
                    (list (getf application :rom))
                    nil))
              (environment
                (list (cons "RETRO_DECK_VOLUME_PERCENT"
                            (format nil "~D" volume-percent)))))
         (unless (eq system :deck)
           (setf environment
                 (append environment
                         (list (cons "RETRO_DECK_EXIT_HINT" "1")))))
         (when wayland
           (setf environment
                 (append environment
                         (list (cons "RETRO_DECK_PRESENTATION"
                                     "layer-shell")))))
         (when (and volume-state (plusp (length volume-state)))
           (setf environment
                 (append environment
                         (list (cons "RETRO_DECK_VOLUME_STATE"
                                     volume-state)))))
         (list :executable (dashboard-executable route)
               :arguments arguments
               :environment environment
               :label (getf application :id)
               :touch-supervision (or wayland (not (eq system :deck)))
               :mirror-console nil))))))

(defun reboot-confirmation-active-p (deadline now)
  (and (plusp deadline) (< now deadline)))
