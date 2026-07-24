(require 'asdf)
(load (or (uiop:getenv "UPLOADER_SOURCE")
          (error "UPLOADER_SOURCE is required")))
(in-package #:retrodeck.uploader)

(let ((stop (pathname (or (uiop:getenv "UPLOADER_STOP")
                          (error "UPLOADER_STOP is required"))))
      (port-file (pathname (or (uiop:getenv "UPLOADER_PORT_FILE")
                               (error "UPLOADER_PORT_FILE is required")))))
  (unwind-protect
       (progn
         (start :address "127.0.0.1"
                :port 0
                :data-root (uiop:getenv "UPLOADER_DATA_ROOT")
                :native (uiop:getenv "UPLOADER_NATIVE")
                :restart-command (list (uiop:getenv "UPLOADER_RESTART"))
                :asset-directory (uiop:getenv "UPLOADER_ASSET_ROOT"))
         (setf *port* (hunchentoot:acceptor-port *acceptor*))
         (with-open-file (stream port-file :direction :output
                                          :if-exists :supersede)
           (format stream "~D~%" *port*))
         (format t "uploader-http-smoke: READY ~D~%" *port*)
         (finish-output)
         (loop until (probe-file stop) do (sleep 0.05)))
    (stop))
  (format t "uploader-http-smoke: STOPPED~%")
  (finish-output)
  (uiop:quit 0))
