# Compiles the Common Lisp tree to a native ARM static library through the
# compiler-enabled ECL running under qemu-arm with the cross toolchain.
{
  lispDir,
  nixpkgsSrc ? builtins.fetchTarball {
    # nixpkgs revision 767b0d3ec98a143ad9ed7dfc0d5553510ac27133
    url = "https://releases.nixos.org/nixpkgs/nixpkgs-26.11pre1031701.767b0d3ec98a/nixexprs.tar.xz";
    sha256 = "sha256-E/v/PHozqkEfjEy5iyvJJ+aQxgH+XV6hOjle67HQ+P4=";
  },
}:

let
  system = "x86_64-linux";
  pkgs = import nixpkgsSrc { inherit system; };
  cross = pkgs.pkgsCross.armv7l-hf-multiplatform;

  eclRuntime = import ./ecl-arm-static.nix {
    compilerSupport = true;
    inherit nixpkgsSrc;
  };
  eclCompiler = eclRuntime.passthru.eclBuild;
  version = eclRuntime.passthru.eclVersion;

  cc = cross.stdenv.cc;
  binutils = cc.bintools.bintools;
  targetPrefix = cc.targetPrefix;
in
pkgs.runCommand "retrodeck-lisp-image-0.1.0" {
  nativeBuildInputs = [ pkgs.qemu-user ];

  passthru = { inherit eclCompiler; };

  meta = {
    description = "RetroDeck Lisp tree compiled to a native ARMv7 archive";
    platforms = [ system ];
  };
} ''
  cp ${lispDir}/*.lisp .
  echo '(pushnew :retrodeck-image *features*)' > image-prologue.lisp

  cat > build-image.lisp <<EOF
  (require :cmp)
  (setf c::*cc* "${cc}/bin/${targetPrefix}cc"
        c::*ld* "${cc}/bin/${targetPrefix}cc"
        c::*ar* "${binutils}/bin/${targetPrefix}ar"
        c::*ranlib* "${binutils}/bin/${targetPrefix}ranlib"
        c::*user-cc-flags* "-I${eclCompiler}/include")
  (pushnew :retrodeck-image *features*)
  (defparameter *units*
    '("image-prologue" "startup" "ui" "policy" "process" "settings"
      "wifi" "credits" "dashboard" "timer" "chiptune"))
  (dolist (unit *units*)
    (let ((source (concatenate 'string unit ".lisp")))
      (format t "~&;; compiling ~A~%" source)
      (finish-output)
      (unless (compile-file source :system-p t
                            :output-file (concatenate 'string unit ".o"))
        (format t "~&;; FAILED ~A~%" source)
        (ext:quit 1))
      (load source :verbose nil :print nil)))
  (c:build-static-library "retrodeck-lisp"
   :lisp-files (mapcar (lambda (unit) (concatenate 'string unit ".o")) *units*)
   :init-name "init_retrodeck_lisp")
  (ext:quit 0)
  EOF

  ECLDIR=${eclCompiler}/lib/ecl-${version}/ \
    qemu-arm ${eclCompiler}/bin/ecl -norc -load build-image.lisp
  test -f libretrodeck-lisp.a
  install -Dm644 libretrodeck-lisp.a $out/lib/libretrodeck-lisp.a
''
