(require 'asdf)
(load (merge-pathnames "../lisp/uploader.lisp"
                       (uiop:pathname-directory-pathname *load-truename*)))
(in-package #:retrodeck.uploader)

(defun expect-request-error (thunk fragment)
  (handler-case
      (progn (funcall thunk) (error "Expected request error containing ~S" fragment))
    (request-error (condition)
      (assert (search fragment (request-error-message condition))))))

(defun write-bytes (path bytes)
  (ensure-directories-exist path)
  (with-open-file (stream path :direction :output :if-exists :supersede
                                :element-type '(unsigned-byte 8))
    (write-sequence bytes stream)))

(defun make-zip (path entries)
  (zip:with-output-to-zipfile (archive path :if-exists :supersede)
    (dolist (entry entries)
      (destructuring-bind (name bytes) entry
        (let ((source (merge-pathnames
                       (format nil "zip-source-~36R" (random most-positive-fixnum))
                       (uiop:pathname-directory-pathname path))))
          (unwind-protect
               (progn
                 (write-bytes source bytes)
                 (with-open-file (stream source :element-type '(unsigned-byte 8))
                   (zip:write-zipentry archive name stream :file-write-date nil)))
            (uiop:delete-file-if-exists source)))))))

(let* ((root (uiop:ensure-directory-pathname
              (format nil "/tmp/retrodeck-uploader-lisp-~36R/"
                      (random most-positive-fixnum))))
       (palette-source (merge-pathnames "../deploy/menu/palette.tsv"
                                        (uiop:pathname-directory-pathname *load-truename*))))
  (unwind-protect
       (progn
         (configure :data-root root :restart-command nil)
         (atomic-text (data-path "nes-deck/menu/games.tsv") "")
         (atomic-text (data-path "nes-deck/menu/palette.tsv")
                      (uiop:read-file-string palette-source))

         (assert (valid-title-p "Café Racer"))
         (assert (not (valid-title-p " padded ")))
         (assert (not (valid-title-p (format nil "~Ctrimmed" (code-char 160)))))
         (assert (string= "hello-world" (slugify "Hello, World!")))
         (assert (string= "#12ABEF" (normalize-rgb "#12abef")))
         (assert (null (normalize-rgb "12ABEF")))
         (clrhash *attempts*)
         (dotimes (attempt 5) (declare (ignore attempt)) (record-login "fixture" nil))
         (assert (= 300 (blocked-seconds "fixture")))
         (record-login "fixture" t)
         (assert (null (blocked-seconds "fixture")))

         (validate-rom "nes" #(78 69 83 26 0 0 0 0 0 0 0 0 0 0 0 0))
         (validate-rom "zx" #(2 0 0 0))
         (validate-rom "chip8" #(1))
         (expect-request-error (lambda () (validate-rom "nes" #(1 2 3 4))) "iNES")
         (expect-request-error (lambda () (validate-rom "zx" #(2 0 0 1))) "checksum")
         (let ((output (make-instance 'bounded-zip-output
                                      :buf (make-array 1 :element-type '(unsigned-byte 8)))))
           (expect-request-error
            (lambda ()
              (trivial-gray-streams:stream-write-sequence output #(1 2) 0 2))
            "read safely"))

         (let* ((palette-text (uiop:read-file-string palette-source))
                (values (parse-palette-tsv palette-text)))
           (assert (= 22 (hash-table-count values)))
           (assert (= 22 (hash-table-count
                          (parse-palette-tsv
                           (format nil "settings-icon~Cgear~%~A" #\Tab palette-text)))))
           (expect-request-error
            (lambda ()
              (parse-palette-tsv
               (format nil "settings-icon~Cgear~%settings-icon~Cgear~%~A"
                       #\Tab #\Tab palette-text)))
            "invalid settings icon")
           (setf (gethash "background" values) "#123ABC")
           (save-palette values)
           (assert (search ":version 2" (uiop:read-file-string
                                         (data-path "nes-deck/state/dashboard-palette.sexp"))))
           (assert (string= "#123ABC" (third (first (current-palette)))))
           (let ((legacy (substitute #\3 #\2 (encode-palette values) :count 1)))
             (setf legacy (concatenate 'string "(:version 3 :settings-icon \"gear\" "
                                       (subseq legacy (length "(:version 2 "))))
             (assert (= 22 (hash-table-count (parse-palette-override legacy))))))

         (let ((catalog (merge-pathnames "long.tsv" root)))
           (atomic-text catalog (format nil "#~A~%" (make-string 4094 :initial-element #\x)))
           (assert (null (parse-catalog catalog)))
           (atomic-text catalog (format nil "#~A~%" (make-string 4095 :initial-element #\x)))
           (expect-request-error (lambda () (parse-catalog catalog)) "token too long"))

         (let ((raw (merge-pathnames "raw.ch8" root)))
           (write-bytes raw #(1 2 3 4))
           (multiple-value-bind (entry restart-error)
               (add-rom "chip8" "Raw Game" "RAW.CH8" raw)
             (assert (null restart-error))
             (assert (string= "upload-chip8-raw-game" (first entry)))
             (assert (probe-file (fourth entry))))
           (expect-request-error
            (lambda () (add-rom "chip8" "Raw Game" "raw.ch8" raw))
            "already cataloged"))

         (let ((archive (merge-pathnames "one.zip" root)))
           (make-zip archive '(("game.ch8" #(9 8 7))))
           (multiple-value-bind (entry restart-error)
               (add-rom "chip8" "Zip Game" "one.zip" archive)
             (assert (null restart-error))
             (assert (string= "Zip Game" (second entry)))))

         (let ((archive (merge-pathnames "many.zip" root)))
           (make-zip archive '(("one.ch8" #(1)) ("two.ch8" #(2))))
           (expect-request-error
            (lambda () (decode-upload "chip8" "many.zip" archive))
            "exactly one ROM"))

         (let ((archive (merge-pathnames "duplicate.zip" root)))
           (make-zip archive '(("game.ch8" #(1)) ("game.ch8" #(2))))
           (expect-request-error
            (lambda () (decode-upload "chip8" "duplicate.zip" archive))
            "exactly one ROM"))

         (let ((archive (merge-pathnames "bad-crc.zip" root)))
           (make-zip archive '(("game.ch8" #(1 2 3))))
           (let* ((bytes (read-bytes archive 10485760))
                  (central (search #(80 75 1 2) bytes)))
             (assert central)
             (setf (aref bytes (+ central 16))
                   (logxor #xff (aref bytes (+ central 16))))
             (write-bytes archive bytes))
           (expect-request-error
            (lambda () (decode-upload "chip8" "bad-crc.zip" archive))
            "read safely"))

         (let ((left (merge-pathnames "left.ch8" root))
               (right (merge-pathnames "right.ch8" root)))
           (write-bytes left #(3))
           (write-bytes right #(4))
           (mapc #'bordeaux-threads:join-thread
                 (list (bordeaux-threads:make-thread
                        (lambda () (add-rom "chip8" "Left Game" "left.ch8" left)))
                       (bordeaux-threads:make-thread
                        (lambda () (add-rom "chip8" "Right Game" "right.ch8" right))))))

         (let ((entries (parse-catalog (data-path "nes-deck/uploads/games.tsv"))))
           (assert (= 4 (length entries)))
           (assert (equal '("Left Game" "Raw Game" "Right Game" "Zip Game")
                          (mapcar #'second entries))))
         (format t "uploader-lisp-test: OK~%"))
    (when (probe-file root)
      (uiop:delete-directory-tree root :validate t :if-does-not-exist :ignore))))
