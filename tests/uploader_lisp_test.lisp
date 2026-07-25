(require 'asdf)
(load (merge-pathnames "../lisp/uploader.lisp"
                       (uiop:pathname-directory-pathname *load-truename*)))
(in-package #:retrodeck.uploader)

(defun expect-request-error (fragment function &rest arguments)
  (handler-case (progn (apply function arguments)
                       (error "Expected request error containing ~S" fragment))
    (request-error (condition) (assert (search fragment (request-error-message condition))))))
(defun write-bytes (path bytes)
  (ensure-directories-exist path)
  (alexandria:write-byte-vector-into-file
   (coerce bytes '(vector (unsigned-byte 8))) path :if-exists :supersede)
  path)
(defun make-zip (path entries)
  (zip:with-output-to-zipfile (archive path :if-exists :supersede)
    (dolist (entry entries)
      (destructuring-bind (name bytes) entry
        (with-open-stream (stream (flexi-streams:make-in-memory-input-stream bytes))
          (zip:write-zipentry archive name stream :file-write-date nil)))))
  path)
(defun add-ok (title filename path expected-id)
  (multiple-value-bind (entry restart-error) (add-rom "zx" title filename path)
    (assert (and (null restart-error) (string= expected-id (first entry))
                 (string= title (second entry)) (probe-file (fourth entry))))))

(let* ((root (uiop:ensure-directory-pathname
              (format nil "/tmp/retrodeck-uploader-lisp-~36R/" (random most-positive-fixnum))))
       (palette-text (uiop:read-file-string
                      (merge-pathnames "../deploy/menu/palette.tsv" *source-directory*))))
  (unwind-protect
       (flet ((path (name) (merge-pathnames name root))
              (file (name bytes) (write-bytes (merge-pathnames name root) bytes)))
         (configure :data-root root :restart-command nil)
         (atomic-text (data-path "nes-deck/menu/games.tsv") "")
         (atomic-text (data-path "nes-deck/menu/palette.tsv") palette-text)

         (assert (and (valid-title-p "Café Racer") (not (valid-title-p " padded "))
                      (not (valid-title-p (format nil "~Ctrimmed" (code-char 160))))
                      (string= "hello-world" (slugify "Hello, World!"))
                      (string= "#12ABEF" (normalize-rgb "#12abef"))
                      (null (normalize-rgb "12ABEF"))))
         (clrhash *attempts*)
         (dotimes (attempt 5) (declare (ignore attempt)) (record-login "fixture" nil))
         (assert (and (= 300 (blocked-seconds "fixture"))
                      (progn (record-login "fixture" t) (null (blocked-seconds "fixture")))))
         (dolist (case '(("nes" #(78 69 83 26 0 0 0 0 0 0 0 0 0 0 0 0))
                         ("zx" #(2 0 0 0))))
           (apply #'validate-rom case))
         (expect-request-error "iNES" #'validate-rom "nes" #(1 2 3 4))
         (expect-request-error "checksum" #'validate-rom "zx" #(2 0 0 1))
         (let ((output (make-instance 'bounded-zip-output
                                      :buf (make-array 1 :element-type '(unsigned-byte 8)))))
           (expect-request-error "read safely" #'trivial-gray-streams:stream-write-sequence
                                 output #(1 2) 0 2))

         (let* ((icon (format nil "settings-icon~Cgear~%" #\Tab))
                (values (parse-palette-tsv palette-text)))
           (assert (= 22 (hash-table-count values)
                      (hash-table-count (parse-palette-tsv (concatenate 'string icon palette-text)))))
           (expect-request-error "invalid settings icon" #'parse-palette-tsv
                                 (concatenate 'string icon icon palette-text))
           (setf (gethash "background" values) "#123ABC")
           (save-palette values)
           (assert (and (search ":version 2" (uiop:read-file-string
                                               (data-path "nes-deck/state/dashboard-palette.sexp")))
                        (string= "#123ABC" (third (first (current-palette))))))
           (assert (= 22 (hash-table-count
                          (parse-palette-override
                           (concatenate 'string "(:version 3 :settings-icon \"gear\" "
                                        (subseq (encode-palette values) (length "(:version 2 "))))))))
         (let ((catalog (path "long.tsv")))
           (atomic-text catalog (format nil "#~A~%" (make-string 4094 :initial-element #\x)))
           (assert (null (parse-catalog catalog)))
           (atomic-text catalog (format nil "#~A~%" (make-string 4095 :initial-element #\x)))
           (expect-request-error "token too long" #'parse-catalog catalog))
         (let ((raw (file "raw.tap" #(2 0 170 170))))
           (add-ok "Raw Game" "RAW.TAP" raw "upload-zx-raw-game")
           (expect-request-error "already cataloged" #'add-rom "zx" "Raw Game" "raw.tap" raw))
         (add-ok "Zip Game" "one.zip"
                 (make-zip (path "one.zip") '(("game.tap" #(2 0 9 9))))
                 "upload-zx-zip-game")
         (dolist (case '(("many.zip" (("one.tap" #(1)) ("two.tap" #(2))))
                         ("duplicate.zip" (("game.tap" #(1)) ("game.tap" #(2))))))
           (destructuring-bind (name entries) case
             (expect-request-error "exactly one ROM" #'decode-upload "zx" name
                                   (make-zip (path name) entries))))
         (let* ((archive (make-zip (path "bad-crc.zip") '(("game.tap" #(1 2 3)))))
                (bytes (read-bytes archive 10485760))
                (central (search #(80 75 1 2) bytes)))
           (assert central)
           (setf (aref bytes (+ central 16)) (logxor #xff (aref bytes (+ central 16))))
           (write-bytes archive bytes)
           (expect-request-error "read safely" #'decode-upload "zx" "bad-crc.zip" archive))

         (let ((left (file "left.tap" #(2 0 5 5)))
               (right (file "right.tap" #(2 0 6 6))))
           (mapc #'bordeaux-threads:join-thread
                 (list (bordeaux-threads:make-thread
                        (lambda () (add-rom "zx" "Left Game" "left.tap" left)))
                       (bordeaux-threads:make-thread
                        (lambda () (add-rom "zx" "Right Game" "right.tap" right))))))
         (let ((entries (parse-catalog (data-path "nes-deck/uploads/games.tsv"))))
           (assert (equal '("Left Game" "Raw Game" "Right Game" "Zip Game")
                          (mapcar #'second entries))))
         (format t "uploader-lisp-test: OK~%"))
    (when (probe-file root)
      (uiop:delete-directory-tree root :validate t :if-does-not-exist :ignore))))
