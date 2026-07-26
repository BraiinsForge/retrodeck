{
  description = "Retro Deck emulators and launcher for Braiins Forge Deck";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    fceumm-src = {
      url = "github:libretro/libretro-fceumm/3a84a6fd0ba20dd4877c06b1d58741172148395f";
      flake = false;
    };
    gambatte-src = {
      url = "github:libretro/gambatte-libretro/dfc165599f3f1068c40a0b7ad6fe5f161283d483";
      flake = false;
    };
    fuse-src = {
      url = "github:libretro/fuse-libretro/bce196fb774835fe65b3e5b821887a4ccf657167";
      flake = false;
    };
    lua-src = {
      url = "https://www.lua.org/ftp/lua-5.5.0.tar.gz";
      flake = false;
    };
  };

  outputs =
    { self, nixpkgs, fenix, fceumm-src, gambatte-src, fuse-src, lua-src }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      pkgsCross = pkgs.pkgsCross.armv7l-hf-multiplatform;
      staticCross = pkgs.pkgsCross.armv7l-hf-multiplatform.pkgsStatic;
      eclArm = import ./nix/ecl-arm-static.nix { };
      retrodeckLispImage = import ./nix/lisp-image.nix { lispDir = ./lisp; };
      eclArmNetwork = import ./nix/ecl-arm-static.nix {
        networkSupport = true;
      };
      uploaderLispLibraries = import ./nix/uploader-lisp-libraries.nix {
        inherit pkgs;
      };
      rustToolchain = fenix.packages.${system}.combine [
        fenix.packages.${system}.stable.cargo
        fenix.packages.${system}.stable.rustc
        fenix.packages.${system}.stable.rust-std
        fenix.packages.${system}.targets.armv7-unknown-linux-gnueabihf.stable.rust-std
      ];
      nativeSources = pkgs.lib.fileset.toSource {
        root = ./.;
        fileset = pkgs.lib.fileset.unions [
          ./native/Cargo.lock
          ./native/Cargo.toml
          ./native/src
          ./protocol/deck-widget-v1.xml
        ];
      };
      nativeCargoDeps = pkgs.rustPlatform.importCargoLock {
        lockFile = ./native/Cargo.lock;
      };

      # Build libgme.a with the glibc cross toolchain; pkgsStatic would mix
      # musl objects into a glibc link.
      gmeStaticCross = pkgsCross.stdenv.mkDerivation {
        pname = "gme-static";
        version = pkgs.game-music-emu.version;
        src = pkgs.game-music-emu.src;
        nativeBuildInputs = [ pkgs.cmake ];
        cmakeFlags = [ "-DBUILD_SHARED_LIBS=OFF" "-DENABLE_UBSAN=OFF" ];
      };
      # Static libretro core archives, one per console, reusing the exact
      # upstream build steps the C++ frontends used.
      fceummCore = pkgsCross.stdenv.mkDerivation {
        pname = "fceumm-core";
        version = "0.1.0-20260714-deck";
        src = fceumm-src;
        nativeBuildInputs = [ pkgs.gnumake ];
        buildInputs = [ pkgsCross.glibc.static staticCross.zlib ];
        NIX_CFLAGS_COMPILE = "-static -O3";
        NIX_LDFLAGS = "-static";
        postPatch = ''
          # A standalone static frontend needs the core's vendored libretro
          # utility implementations instead of symbols from RetroArch.
          substituteInPlace Makefile.common \
            --replace-fail \
              'ifneq ($(STATIC_LINKING), 1)' \
              'ifeq ($(STATIC_LINKING), 1)'
        '';
        buildPhase = ''
          runHook preBuild
          make -j$NIX_BUILD_CORES \
            platform=rpi2 \
            STATIC_LINKING=1 \
            TARGET=fceumm_libretro.a \
            EXTERNAL_ZLIB=1 \
            CC=$CC \
            AR=${pkgsCross.stdenv.cc.targetPrefix}ar
          runHook postBuild
        '';
        installPhase = ''
          runHook preInstall
          install -Dm644 fceumm_libretro.a $out/lib/libfceumm.a
          install -Dm644 Copying \
            $out/share/licenses/nes-deck/FCEUmm-COPYING
          runHook postInstall
        '';
      };
      gambatteCore = pkgsCross.stdenv.mkDerivation {
        pname = "gambatte-core";
        version = "0.5.0-20260703-deck";
        src = gambatte-src;
        nativeBuildInputs = [ pkgs.gnumake ];
        buildInputs = [ pkgsCross.glibc.static ];
        NIX_CFLAGS_COMPILE = "-static -O3";
        NIX_LDFLAGS = "-static";
        postPatch = ''
          # Preserve Gambatte's include/feature flags while replacing its
          # generic -O2 release setting with the Deck SoC tuning used by
          # the project's own Cortex-A7 targets.
          substituteInPlace Makefile.libretro \
            --replace-fail \
              'CFLAGS   += -O2 -DNDEBUG' \
              'CFLAGS   += -Ofast -flto -ffat-lto-objects -fomit-frame-pointer -fno-math-errno -marm -march=armv7-a -mtune=cortex-a7 -mfpu=neon-vfpv4 -mfloat-abi=hard -DNDEBUG' \
            --replace-fail \
              'CXXFLAGS += -O2 -DNDEBUG' \
              'CXXFLAGS += -Ofast -flto -ffat-lto-objects -fomit-frame-pointer -fno-math-errno -marm -march=armv7-a -mtune=cortex-a7 -mfpu=neon-vfpv4 -mfloat-abi=hard -DNDEBUG'
          # Include the core's vendored libretro utility implementations.
          substituteInPlace Makefile.common \
            --replace-fail \
              'ifneq ($(STATIC_LINKING), 1)' \
              'ifeq ($(STATIC_LINKING), 1)'
        '';
        buildPhase = ''
          runHook preBuild
          make \
            STATIC_LINKING=1 \
            platform=unix \
            TARGET=gambatte_libretro.a \
            CC=$CC \
            CXX=$CXX \
            AR=${pkgsCross.stdenv.cc.targetPrefix}ar \
            fpic= \
            HAVE_NETWORK=0
          runHook postBuild
        '';
        installPhase = ''
          runHook preInstall
          install -Dm644 gambatte_libretro.a $out/lib/libgambatte.a
          install -Dm644 COPYING \
            $out/share/licenses/gb-deck/Gambatte-COPYING
          runHook postInstall
        '';
      };
      fuseCore = pkgsCross.stdenv.mkDerivation {
        pname = "fuse-core";
        version = "1.6.0-20260420-deck";
        src = fuse-src;
        nativeBuildInputs = [ pkgs.gnumake ];
        buildInputs = [ pkgsCross.glibc.static ];
        NIX_CFLAGS_COMPILE = "-static -O3";
        NIX_LDFLAGS = "-static";
        postPatch = ''
          # The Nix source has no Git metadata. Generate the version source
          # once from the pinned revision instead of invoking git.
          substituteInPlace Makefile.libretro \
            --replace-fail \
              '$(CORE_DIR)/src/version.c: FORCE' \
              '$(CORE_DIR)/src/version.c:'
        '';
        buildPhase = ''
          runHook preBuild
          sed 's/HASH/bce196fb774835fe65b3e5b821887a4ccf657167/' \
            etc/version.c.templ > src/version.c
          make -f Makefile.libretro -j$NIX_BUILD_CORES \
            platform=rpi2 \
            STATIC_LINKING=1 \
            TARGET=fuse_libretro.a \
            CC=$CC \
            CXX=$CXX \
            AR=${pkgsCross.stdenv.cc.targetPrefix}ar
          runHook postBuild
        '';
        installPhase = ''
          runHook preInstall
          install -Dm644 fuse_libretro.a $out/lib/libfuse.a
          install -Dm644 LICENSE $out/share/licenses/zx-deck/Fuse-LICENSE
          install -Dm644 libspectrum/COPYING \
            $out/share/licenses/zx-deck/libspectrum-COPYING
          install -Dm644 bzip2/LICENSE \
            $out/share/licenses/zx-deck/bzip2-LICENSE
          runHook postInstall
        '';
      };
      # One Rust libretro host binary per console, with the core linked in.
      retroHost = { name, core, coreDerivation, coreLibrary, description, license
        , linkTimeOptimization ? false }:
        pkgs.stdenvNoCC.mkDerivation {
          pname = name;
          version = "1.0.0";
          src = nativeSources;
          cargoDeps = nativeCargoDeps;
          cargoRoot = "native";
          nativeBuildInputs = [
            rustToolchain
            pkgs.rustPlatform.cargoSetupHook
            pkgsCross.stdenv.cc
            pkgs.nukeReferences
          ];
          buildInputs = [
            pkgsCross.glibc.static
            staticCross.libvorbis
            staticCross.zlib
            gmeStaticCross
            coreDerivation
          ];
          allowedReferences = [ ];
          RETRO_DECK_CORE = core;
          buildPhase = ''
            runHook preBuild
            cd native
            export CARGO_HOME=$TMPDIR/cargo
            export CARGO_BUILD_TARGET=armv7-unknown-linux-gnueabihf
            export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER="${pkgsCross.stdenv.cc}/bin/${pkgsCross.stdenv.cc.targetPrefix}cc"
            export RUSTFLAGS="\
              -C target-feature=+crt-static,+v7,+hwdiv,+hwdiv-arm \
              -C link-arg=-static \
              -L native=${coreDerivation}/lib \
              -L native=${pkgsCross.glibc.static}/lib \
              -l static=${coreLibrary} \
              -l z \
              -l m \
              ${pkgs.lib.optionalString linkTimeOptimization "-C link-arg=-flto"}"
            cargo build --release --locked --offline --bin retro-host
            runHook postBuild
          '';
          installPhase = ''
            runHook preInstall
            install -Dm755 \
              target/armv7-unknown-linux-gnueabihf/release/retro-host \
              $out/bin/${name}
            mkdir -p $out/share/licenses
            cp -a ${coreDerivation}/share/licenses/${name} \
              $out/share/licenses/${name}
            nuke-refs $out/bin/${name}
            runHook postInstall
          '';
          meta = {
            inherit description license;
            platforms = [ system ];
          };
        };
      runtimeLicenses = import ./nix/runtime-licenses.nix {
        inherit pkgs pkgsCross staticCross nativeCargoDeps;
        nixpkgsSource = nixpkgs.outPath;
      };

    in
    {
      packages.${system} = {
        ecl-arm-network = eclArmNetwork;
        retrodeck-lisp-image = retrodeckLispImage;
        uploader-lisp-libraries = uploaderLispLibraries;

        retrodeck-native = pkgs.stdenvNoCC.mkDerivation {
          pname = "retrodeck-native";
          version = "1.0.0";

          src = nativeSources;
          cargoDeps = nativeCargoDeps;
          cargoRoot = "native";
          nativeBuildInputs = [
            rustToolchain
            pkgs.rustPlatform.cargoSetupHook
            pkgsCross.stdenv.cc
            pkgs.nukeReferences
          ];
          buildInputs = [
            pkgsCross.glibc.static
            staticCross.libvorbis
            gmeStaticCross
          ];
          allowedReferences = [ ];

          buildPhase = ''
            runHook preBuild
            cd native
            export CARGO_HOME=$TMPDIR/cargo
            export CARGO_BUILD_TARGET=armv7-unknown-linux-gnueabihf
            export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER="${pkgsCross.stdenv.cc}/bin/${pkgsCross.stdenv.cc.targetPrefix}cc"
            export RUSTFLAGS="\
              -C target-feature=+crt-static,+v7,+hwdiv,+hwdiv-arm \
              -C link-arg=-static \
              -L native=${retrodeckLispImage}/lib \
              -L native=${eclArm.dev}/lib \
              -L native=${pkgsCross.glibc.static}/lib \
              -l static=retrodeck-lisp \
              -l static=ecl \
              -l static=eclgc \
              -l static=gmp \
              -l dl \
              -l m"
            cargo build --release --locked --offline --features lisp-image
            runHook postBuild
          '';

          installPhase = ''
            runHook preInstall
            install -Dm755 \
              target/armv7-unknown-linux-gnueabihf/release/retrodeck-native \
              $out/bin/retrodeck-native
            nuke-refs $out/bin/retrodeck-native
            runHook postInstall
          '';

          meta = {
            description = "Rust mechanism host for the RetroDeck Lisp orchestrator";
            platforms = [ system ];
          };
        };

        runtime-licenses = runtimeLicenses;

        nes-deck = retroHost {
          name = "nes-deck";
          core = "nes";
          coreDerivation = fceummCore;
          coreLibrary = "fceumm";
          description = "FCEUmm NES core with the Rust Deck frontend";
          license = pkgs.lib.licenses.gpl2Only;
        };

        gb-deck = retroHost {
          name = "gb-deck";
          core = "gb";
          coreDerivation = gambatteCore;
          coreLibrary = "gambatte";
          description = "Gambatte GB/GBC core with the Rust Deck frontend";
          license = pkgs.lib.licenses.gpl2Only;
        };

        zx-deck = retroHost {
          name = "zx-deck";
          core = "zx";
          coreDerivation = fuseCore;
          coreLibrary = "fuse";
          description = "Fuse ZX Spectrum core with the Rust Deck frontend";
          license = pkgs.lib.licenses.gpl3Only;
        };

        lua-deck = pkgsCross.stdenv.mkDerivation {
          pname = "lua-deck";
          version = "5.5.0";

          src = lua-src;
          nativeBuildInputs = [ pkgs.gnumake pkgs.nukeReferences ];
          buildInputs = [ pkgsCross.glibc.static ];
          allowedReferences = [ ];

          NIX_CFLAGS_COMPILE = "-Os";
          NIX_LDFLAGS = "-static";

          buildPhase = ''
            runHook preBuild
            make -C src -j$NIX_BUILD_CORES posix \
              CC="$CC -std=gnu99" \
              AR="${pkgsCross.stdenv.cc.targetPrefix}ar rcu" \
              RANLIB=${pkgsCross.stdenv.cc.targetPrefix}ranlib \
              MYCFLAGS="-Os -ffunction-sections -fdata-sections" \
              MYLDFLAGS="-static -Wl,--gc-sections -Wl,-s"
            runHook postBuild
          '';

          installPhase = ''
            runHook preInstall
            mkdir -p $out/bin $out/share/licenses/lua-deck
            install -m755 src/lua $out/bin/lua
            install -m644 doc/readme.html \
              $out/share/licenses/lua-deck/LICENSE.html
            nuke-refs $out/bin/lua
            runHook postInstall
          '';

          meta = {
            description = "Static Lua interpreter for the Braiins Forge Deck";
            homepage = "https://www.lua.org/";
            license = pkgs.lib.licenses.mit;
            platforms = [ "armv7l-linux" ];
          };
        };

        python-deck = pkgsCross.stdenv.mkDerivation {
          pname = "python-deck";
          version = pkgs.micropython.version;

          src = pkgs.micropython.src;
          nativeBuildInputs = [ pkgs.gnumake pkgs.python3 pkgs.nukeReferences ];
          buildInputs = [ pkgsCross.glibc.static ];
          allowedReferences = [ ];

          NIX_CFLAGS_COMPILE = "-Os";
          NIX_LDFLAGS = "-static";

          buildPhase = ''
            runHook preBuild
            make -C ports/unix -j$NIX_BUILD_CORES \
              VARIANT=minimal \
              CROSS_COMPILE=${pkgsCross.stdenv.cc.targetPrefix} \
              CC=$CC \
              STRIP=${pkgsCross.stdenv.cc.targetPrefix}strip \
              CFLAGS_EXTRA="-Os -ffunction-sections -fdata-sections" \
              LDFLAGS_EXTRA="-static -Wl,--gc-sections"
            runHook postBuild
          '';

          installPhase = ''
            runHook preInstall
            mkdir -p $out/bin $out/share/licenses/python-deck
            install -m755 ports/unix/build-minimal/micropython \
              $out/bin/python
            install -m644 LICENSE \
              $out/share/licenses/python-deck/LICENSE
            nuke-refs $out/bin/python
            runHook postInstall
          '';

          meta = {
            description = "Static MicroPython REPL for the Braiins Forge Deck";
            homepage = "https://micropython.org/";
            license = pkgs.lib.licenses.mit;
            platforms = [ "armv7l-linux" ];
          };
        };

        chibi-deck = pkgsCross.stdenv.mkDerivation {
          pname = "chibi-deck";
          version = pkgs.chibi.version;

          src = pkgs.chibi.src;
          patches = [ ./patches/chibi-static-module-path.patch ];
          nativeBuildInputs = [ pkgs.gnumake pkgs.chibi pkgs.nukeReferences ];
          buildInputs = [ pkgsCross.glibc.static ];
          allowedReferences = [ ];

          NIX_CFLAGS_COMPILE = "-Os";
          NIX_LDFLAGS = "-static";

          buildPhase = ''
            runHook preBuild
            make -j$NIX_BUILD_CORES clibs.c \
              PLATFORM=linux \
              CHIBI_DEPENDENCIES= \
              CHIBI="${pkgs.chibi}/bin/chibi-scheme -I ./lib" \
              CHIBI_FFI="${pkgs.chibi}/bin/chibi-scheme -I ./lib -q tools/chibi-ffi"
            make -j$NIX_BUILD_CORES chibi-scheme-static \
              PLATFORM=linux \
              ARCH=armv7l \
              CC=$CC \
              AR=${pkgsCross.stdenv.cc.targetPrefix}ar \
              CPPFLAGS="-DSEXP_USE_DL=0 -DSEXP_USE_STATIC_LIBS=1 -DSEXP_USE_STATIC_LIBS_NO_INCLUDE=0" \
              CFLAGS="-Os -ffunction-sections -fdata-sections" \
              LDFLAGS="-static -Wl,--gc-sections -Wl,-s"
            runHook postBuild
          '';

          installPhase = ''
            runHook preInstall
            mkdir -p $out/bin $out/share/chibi \
              $out/share/licenses/chibi-deck
            install -m755 chibi-scheme-static $out/bin/chibi-scheme
            cp -R ${pkgs.chibi}/share/chibi/. $out/share/chibi/
            chmod -R u+w $out/share/chibi
            find $out/share/chibi -type f \
              \( -name '*.so' -o -name '*.img' \) -delete
            cd ${pkgs.chibi}/lib/chibi
            find . -type f -name '*.so' -print | while read module; do
              mkdir -p "$out/share/chibi/$(dirname "$module")"
              install -m444 /dev/null "$out/share/chibi/$module"
            done
            cd - >/dev/null
            install -m644 COPYING \
              $out/share/licenses/chibi-deck/COPYING
            nuke-refs $out/bin/chibi-scheme
            runHook postInstall
          '';

          meta = {
            description = "Static Chibi Scheme REPL for the Braiins Forge Deck";
            homepage = "https://github.com/ashinn/chibi-scheme";
            license = pkgs.lib.licenses.bsd3;
            platforms = [ "armv7l-linux" ];
          };
        };

        rlwrap-deck = staticCross.rlwrap.overrideAttrs (old: {
          pname = "rlwrap-deck";
          nativeBuildInputs = (old.nativeBuildInputs or []) ++
            [ pkgs.nukeReferences ];
          allowedReferences = [ ];

          postInstall = (old.postInstall or "") + ''
            rm -rf $out/share
            mkdir -p $out/share/licenses/rlwrap-deck
            install -m644 COPYING $out/share/licenses/rlwrap-deck/COPYING
          '';

          postFixup = (old.postFixup or "") + ''
            rm -rf $out/nix-support
            nuke-refs $out/bin/rlwrap
          '';

          meta = (old.meta or {}) // {
            description = "Static rlwrap for the Deck Lisp REPL";
            platforms = [ "armv7l-linux" ];
          };
        });

        fbterm-deck = staticCross.fbterm.overrideAttrs (old: {
          pname = "fbterm-deck";
          version = "1.7-deck";
          src = ./terminal/fbterm;

          # The Deck has no pointer-driven terminal UI, and static gpm in the
          # pinned nixpkgs leaves dangling shared-library symlinks.  Keep the
          # terminal fully static by disabling that optional integration.
          configureFlags = (old.configureFlags or []) ++ [ "--disable-gpm" ];
          nativeBuildInputs = (old.nativeBuildInputs or []) ++
            [ pkgs.nukeReferences ];
          propagatedBuildInputs = builtins.filter
            (dependency: dependency != staticCross.gpm)
            (old.propagatedBuildInputs or []);
          allowedReferences = [ ];

          postInstall = (old.postInstall or "") + ''
            mkdir -p $out/share/retro-deck/fonts \
              $out/share/retro-deck/keymaps \
              $out/share/licenses/fbterm-deck
            install -m755 ${staticCross.kbd}/bin/loadkeys $out/bin/loadkeys
            install -m644 \
              ${pkgs.dejavu_fonts}/share/fonts/truetype/DejaVuSansMono.ttf \
              $out/share/retro-deck/fonts/DejaVuSansMono.ttf
            ${pkgs.gzip}/bin/gzip -dc \
              ${pkgs.kbd}/share/keymaps/i386/qwerty/us.map.gz \
              > $out/share/retro-deck/keymaps/us.map
            ${pkgs.gzip}/bin/gzip -dc \
              ${pkgs.kbd}/share/keymaps/i386/qwertz/cz-qwertz.map.gz \
              > $out/share/retro-deck/keymaps/cz.map
            install -m644 \
              ${pkgs.kbd}/share/keymaps/i386/include/qwerty-layout.inc \
              ${pkgs.kbd}/share/keymaps/i386/include/compose.inc \
              ${pkgs.kbd}/share/keymaps/i386/include/linux-with-alt-and-altgr.inc \
              ${pkgs.kbd}/share/keymaps/i386/include/linux-keys-bare.inc \
              ${pkgs.kbd}/share/keymaps/include/compose.latin1 \
              $out/share/retro-deck/keymaps/
            ${pkgs.gzip}/bin/gzip -dc \
              ${pkgs.kbd}/share/keymaps/i386/include/euro1.map.gz \
              > $out/share/retro-deck/keymaps/euro1.map
            install -m644 COPYING $out/share/licenses/fbterm-deck/COPYING
            install -m644 ${./terminal/fonts/DejaVu-LICENSE} \
              $out/share/licenses/fbterm-deck/DejaVu-LICENSE
            tar --extract --xz --to-stdout --file=${pkgs.kbd.src} \
              --wildcards 'kbd-*/COPYING' \
              > $out/share/licenses/fbterm-deck/kbd-COPYING
          '';

          postFixup = (old.postFixup or "") + ''
            rm -rf $out/etc $out/nix-support/propagated-build-inputs
            nuke-refs $out/bin/fbterm $out/bin/loadkeys
          '';

          meta = (old.meta or {}) // {
            description = "Padded Deck fbterm with scoped US and Czech keymaps";
            platforms = [ "armv7l-linux" ];
          };
        });

        default = self.packages.${system}.nes-deck;
      };

      checks.${system} = {
        retrodeck-native-smoke = pkgs.runCommand "retrodeck-native-smoke" { } ''
          cp ${./lisp/startup.lisp} startup.lisp
          cp ${./lisp/ui.lisp} ui.lisp
          cp ${./lisp/timer.lisp} timer.lisp
          cp ${./lisp/policy.lisp} policy.lisp
          cp ${./lisp/chiptune.lisp} chiptune.lisp
          cp ${./chiptunes/crazy.ogg} crazy.ogg
          cp ${./lisp/process.lisp} process.lisp
          cp ${./lisp/settings.lisp} settings.lisp
          cp ${./lisp/wifi.lisp} wifi.lisp
          cp ${./lisp/credits.lisp} credits.lisp
          cp ${./lisp/dashboard.lisp} dashboard.lisp
          cp ${./assets/settings-cog/gear-knekko-09.png} settings-icon.png
          cat > terminal-fixture <<'EOF'
          #!${pkgs.runtimeShell}
          [ "$#" -eq 1 ] || exit 90
          [ "$RETRO_DECK_KEYMAP" = cz ] || exit 92
          case "$1" in
            shell) exit 0 ;;
            failure) exit 7 ;;
            signal) kill -TERM "$$" ;;
            *) exit 91 ;;
          esac
          EOF
          chmod +x terminal-fixture
          cat > child-fixture <<'EOF'
          #!${pkgs.runtimeShell}
          [ "$#" -eq 2 ] || exit 90
          [ "$2" = "second argument" ] || exit 91
          [ "$RETRODECK_ALPHA" = "alpha value" ] || exit 92
          [ "$RETRODECK_BETA" = beta ] || exit 93
          case "$1" in
            clean) exit 0 ;;
            failure) exit 7 ;;
            signal) kill -TERM "$$" ;;
            *) exit 94 ;;
          esac
          EOF
          cat > reboot-fixture <<'EOF'
          #!${pkgs.runtimeShell}
          [ "$#" -eq 0 ] || exit 90
          exit 0
          EOF
          chmod +x child-fixture reboot-fixture
          cat > helper-fixture <<'EOF'
          #!${pkgs.runtimeShell}
          [ "$#" -eq 0 ] || exit 90
          cat > helper-capture
          EOF
          cat > helper-failure-fixture <<'EOF'
          #!${pkgs.runtimeShell}
          [ "$#" -eq 0 ] || exit 90
          cat >/dev/null
          exit 7
          EOF
          cat > helper-signal-fixture <<'EOF'
          #!${pkgs.runtimeShell}
          [ "$#" -eq 0 ] || exit 90
          cat >/dev/null
          kill -TERM "$$"
          EOF
          cat > helper-reject-fixture <<'EOF'
          #!${pkgs.runtimeShell}
          [ "$#" -eq 0 ] || exit 90
          exit 7
          EOF
          chmod +x helper-fixture helper-failure-fixture \
            helper-signal-fixture helper-reject-fixture
          printf 'CONNECTED\n' > wifi-status
          rm -f helper-capture state-file state-file.keymap state-file.control \
            state-file.brightness state-file.brightness-max \
            state-file.brightness-state
          printf '12\n' > state-file.control
          printf '12\n' > state-file.brightness
          printf '20\n' > state-file.brightness-max
          printf '%b' 'id\ttitle\tsystem\trom\t#RRGGBB\r\nupload-one\t\0305\0275lu\0305\0245\tgb\t/missing/upload.gb\t#87AFAF\r\n' \
            > combined-games.tsv
          printf '%b' 'bad-utf8\t\0300\0257\tnes\t/missing/bad.nes\t#D78787\n' \
            > invalid-games.tsv
          substitute ${./tests/native_ecl_smoke.lisp.in} smoke.lisp \
            --subst-var-by startup "$PWD/startup.lisp" \
            --subst-var-by state_file "$PWD/state-file" \
            --subst-var-by wifi_status "$PWD/wifi-status" \
            --subst-var-by settings_icon "$PWD/settings-icon.png" \
            --subst-var-by terminal_fixture "$PWD/terminal-fixture" \
            --subst-var-by child_fixture "$PWD/child-fixture" \
            --subst-var-by reboot_fixture "$PWD/reboot-fixture" \
            --subst-var-by helper_fixture "$PWD/helper-fixture" \
            --subst-var-by helper_failure_fixture "$PWD/helper-failure-fixture" \
            --subst-var-by helper_signal_fixture "$PWD/helper-signal-fixture" \
            --subst-var-by helper_reject_fixture "$PWD/helper-reject-fixture" \
            --subst-var-by helper_capture "$PWD/helper-capture" \
            --subst-var-by games ${./deploy/menu/games.tsv} \
            --subst-var-by palette ${./deploy/menu/palette.tsv} \
            --subst-var-by combined_games "$PWD/combined-games.tsv" \
            --subst-var-by invalid_games "$PWD/invalid-games.tsv" \
            --subst-var-by credits ${./deploy/menu/credits.tsv} \
            --subst-var-by chiptune "$PWD/crazy.ogg"
          WAYLAND_DISPLAY=retrodeck-smoke \
            XDG_RUNTIME_DIR=/tmp \
            RETRO_DECK_VOLUME_PERCENT=0 \
            RETRO_DECK_REDUCED_MOTION=1 \
            ECLDIR=${eclArm}/lib/ecl/ \
            ${pkgs.qemu-user}/bin/qemu-arm \
            ${self.packages.${system}.retrodeck-native}/bin/retrodeck-native \
            smoke.lisp
          # Without arguments the binary must come up from the compiled-in
          # Lisp image with no sources on disk at all.
          ${pkgs.qemu-user}/bin/qemu-arm \
            ${self.packages.${system}.retrodeck-native}/bin/retrodeck-native \
            > image.log
          grep -q "retrodeck: Common Lisp startup loaded" image.log
          touch $out
        '';

        libretro-host-smoke = pkgs.runCommand "libretro-host-smoke" { } ''
          run() {
            rom=$(basename "$2")
            cp "$2" "$rom"
            chmod 600 "$rom"
            RETRO_DECK_TEST_FRAMES=120 RETRO_DECK_VOLUME_PERCENT=0 \
              ${pkgs.qemu-user}/bin/qemu-arm "$1" "$rom" > run.log
            cat run.log
            grep -q "test frames=120 video=120 " run.log
            grep -q "hash=$3" run.log
          }
          run ${self.packages.${system}.nes-deck}/bin/nes-deck \
            ${./roms/nes/micro-mages.nes} 93262b846f382c60
          run ${self.packages.${system}.gb-deck}/bin/gb-deck \
            ${./roms/gb/kirbys-dream-land.gb} 9e145219789c4817
          run ${self.packages.${system}.zx-deck}/bin/zx-deck \
            ${./roms/zx/knight-lore.tap} 9ca35bdc8ecfa26d
          touch $out
        '';

        uploader-password-smoke = pkgs.runCommand "uploader-password-smoke" { } ''
          cat > password.conf <<'EOF'
          version=1
          iterations=100000
          salt=AAECAwQFBgcICQoLDA0ODw
          digest=d43qgN/WlhHSRiEo/5Z7C52SrUrDpGclHZKmaPtBmTo
          EOF
          chmod 600 password.conf
          helper=${self.packages.${system}.retrodeck-native}/bin/retrodeck-native
          qemu=${pkgs.qemu-user}/bin/qemu-arm
          printf configured-test-password | $qemu $helper \
            --verify-uploader-password password.conf
          if printf wrong-password | $qemu $helper \
              --verify-uploader-password password.conf; then
            echo "uploader password helper accepted a wrong password" >&2
            exit 1
          fi
          chmod 644 password.conf
          if printf configured-test-password | $qemu $helper \
              --verify-uploader-password password.conf; then
            echo "uploader password helper accepted a public config" >&2
            exit 1
          fi
          sed 's/$/\r/' password.conf > password-crlf.conf
          cp password.conf password-extra-newline.conf
          printf '\n' >> password-extra-newline.conf
          chmod 600 password-crlf.conf password-extra-newline.conf
          for malformed in password-crlf.conf password-extra-newline.conf; do
            if printf configured-test-password | $qemu $helper \
                --verify-uploader-password "$malformed"; then
              echo "uploader password helper accepted $malformed" >&2
              exit 1
            fi
          done
          touch $out
        '';

        uploader-lisp-policy = pkgs.runCommand "uploader-lisp-policy" {
          nativeBuildInputs = [ pkgs.sbcl ];
        } ''
          mkdir -p source/{deploy/menu,lisp,tests} home
          cp ${./deploy/menu/palette.tsv} source/deploy/menu/palette.tsv
          cp ${./lisp/uploader.lisp} source/lisp/uploader.lisp
          cp ${./tests/uploader_lisp_test.lisp} source/tests/uploader_lisp_test.lisp
          cd source
          HOME=$NIX_BUILD_TOP/home \
            CL_SOURCE_REGISTRY=${uploaderLispLibraries}/share/common-lisp/source//: \
            sbcl --noinform --disable-debugger --script tests/uploader_lisp_test.lisp
          HOME=$NIX_BUILD_TOP/home \
            XDG_CACHE_HOME=$NIX_BUILD_TOP/home/cache \
            CL_SOURCE_REGISTRY=${uploaderLispLibraries}/share/common-lisp/source//: \
            ECLDIR=${eclArmNetwork}/lib/ecl/ \
            ${pkgs.qemu-user}/bin/qemu-arm ${eclArmNetwork}/bin/ecl.bin \
            -norc -load tests/uploader_lisp_test.lisp -eval '(ext:quit 0)'
          touch $out
        '';

        ecl-arm-network-smoke = pkgs.runCommand "ecl-arm-network-smoke" { } ''
          cat > smoke.lisp <<'EOF'
          (unless (member :threads *features*)
            (error "network ECL lacks thread support"))
          (let ((complete nil))
            (mp:process-join
             (mp:process-run-function "network-ecl-smoke"
                                      (lambda () (setf complete t))))
            (unless complete
              (error "network ECL thread probe diverged")))
          (require 'serve-event)
          (unless (null (serve-event:serve-all-events 0))
            (error "network ECL serve-event probe diverged"))
          (let ((socket (make-instance 'sb-bsd-sockets:inet-socket
                                       :type :stream :protocol 6)))
            (unwind-protect
                (progn
                  (sb-bsd-sockets:socket-bind socket #(127 0 0 1) 0)
                  (sb-bsd-sockets:socket-listen socket 1)
                  (multiple-value-bind (address port)
                      (sb-bsd-sockets:socket-name socket)
                    (unless (and (equalp address #(127 0 0 1)) (plusp port))
                      (error "network ECL socket probe diverged"))))
              (sb-bsd-sockets:socket-close socket)))
          (format t "network-ecl-smoke: OK~%")
          (ext:quit 0)
          EOF
          ECLDIR=${eclArmNetwork}/lib/ecl/ \
            ${pkgs.qemu-user}/bin/qemu-arm \
            ${eclArmNetwork}/bin/ecl.bin -norc -load smoke.lisp
          touch $out
        '';

        uploader-lisp-http-smoke = pkgs.runCommand "uploader-lisp-http-smoke" {
          nativeBuildInputs = [ pkgs.bash pkgs.curl pkgs.sbcl pkgs.zip ];
        } ''
          export CL_SOURCE_REGISTRY=${uploaderLispLibraries}/share/common-lisp/source//:
          export UPLOADER_SOURCE=${./lisp/uploader.lisp}
          export UPLOADER_SERVER=${./tests/uploader_http_smoke.lisp}
          export UPLOADER_ASSET_ROOT=${./lisp}
          export UPLOADER_PALETTE=${./deploy/menu/palette.tsv}
          export UPLOADER_SHELL=${pkgs.bash}/bin/bash
          ${pkgs.bash}/bin/bash ${./tests/uploader-http-smoke.sh} host \
            ${pkgs.sbcl}/bin/sbcl --noinform --disable-debugger --script
          ECLDIR=${eclArmNetwork}/lib/ecl/ \
            ${pkgs.bash}/bin/bash ${./tests/uploader-http-smoke.sh} arm-ecl \
            ${pkgs.qemu-user}/bin/qemu-arm \
            ${eclArmNetwork}/bin/ecl.bin -norc -load
          touch $out
        '';
      };

      devShells.${system}.default = pkgs.mkShell {
        nativeBuildInputs = [
          pkgsCross.stdenv.cc
          pkgs.gnumake
        ];

        buildInputs = [
          pkgsCross.glibc.static
        ];

        shellHook = ''
          export CROSS_COMPILE="${pkgsCross.stdenv.cc.targetPrefix}"
          export CC="${pkgsCross.stdenv.cc}/bin/${pkgsCross.stdenv.cc.targetPrefix}gcc"
          export CXX="${pkgsCross.stdenv.cc}/bin/${pkgsCross.stdenv.cc.targetPrefix}g++"
          export CFLAGS="-static -O3 -fsigned-char"
          export LDFLAGS="-static -lpthread -lm"

          echo "Retro Deck cross-compile environment for Braiins Forge Deck"
          echo ""
          echo "Environment configured:"
          echo "  CROSS_COMPILE=$CROSS_COMPILE"
          echo "  CC=$CC"
          echo "  Target: armv7l-hf"
          echo ""
        '';
      };
    };
}
