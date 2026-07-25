;;; Service entry for the Lisp ROM uploader. The init script provides
;;; ECLDIR and CL_SOURCE_REGISTRY so ASDF can find Hunchentoot's sources.
(require 'asdf)
(let ((here (make-pathname :name nil :type nil :defaults *load-truename*)))
  (load (merge-pathnames "uploader.lisp" here) :verbose nil :print nil))
(retrodeck.uploader:run)
