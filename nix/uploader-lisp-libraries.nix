{ pkgs }:

let
  wrap = package: {
    key = package.outPath;
    inherit package;
  };
  closure = pkgs.lib.genericClosure {
    startSet = map wrap [ pkgs.sbclPackages.hunchentoot pkgs.sbclPackages.zip ];
    operator = item:
      map wrap (builtins.filter
        (package: package.pname != "cl+ssl")
        (item.package.lispLibs or [ ]));
  };
  packages = map (item: item.package) closure;
in
pkgs.runCommand "retrodeck-uploader-lisp-libraries" {
  allowedReferences = [ ];
  passthru.packageNames = map (package: package.pname) packages;
  meta = {
    description = "Source libraries for the ECL RetroDeck uploader";
    platforms = pkgs.lib.platforms.all;
  };
} ''
  source=$out/share/common-lisp/source
  mkdir -p "$source/sb-bsd-sockets"
  ${pkgs.lib.concatMapStringsSep "\n" (package: ''
    cp -R ${package.src} "$source/${package.pname}"
    echo ${package.pname} >> "$out/packages"
  '') packages}

  # Keep runtime source and license notices, not upstream docs and test data.
  chmod -R u+w "$source"
  rm -rf "$source"/*/{doc,docs,doc-src,test,tests,website}
  find "$source" -type f \
    ! -name '*.asd' ! -name '*.lisp' ! -name '*.sexp' \
    ! -iname 'copying*' ! -iname 'license*' ! -iname 'licence*' \
    ! -iname 'notice*' ! -iname 'readme*' ! -iname 'authors*' \
    -delete
  find "$source" -depth -type d -empty -delete

  # ECL provides the SB-BSD-SOCKETS package without an ASDF system.
  mkdir -p "$source/sb-bsd-sockets"
  cat > "$source/sb-bsd-sockets/sb-bsd-sockets.asd" <<'EOF'
  (asdf:defsystem #:sb-bsd-sockets)
  EOF

  # ECL accepts numeric protocols without consulting /etc/protocols.
  substituteInPlace "$source/usocket/backend/sbcl.lisp" \
    --replace-fail '#+(or ecl mkcl clasp) :tcp' \
                   '#+ecl 6 #+(or mkcl clasp) :tcp' \
    --replace-fail ':protocol (if (pathnamep host) 0 :tcp)' \
                   ':protocol (if (pathnamep host) 0 #+ecl 6 #-ecl :tcp)'
''
