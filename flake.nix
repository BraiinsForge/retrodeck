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
    gpsp-src = {
      url = "github:libretro/gpsp/5b6e751f4abf368509146cd143c949c1946ac1ae";
      flake = false;
    };
    # The Deck fbDOOM fork. Its framebuffer and tty backends are replaced by
    # the Retro Deck platform layer in native/doom, so only the engine and
    # the fork's multiplayer branch are used from here.
    fbdoom-src = {
      url = "github:BraiinsForge/fbDOOM-fork/26ea06ff3656e477019641d45b4c7d9066887503";
      flake = false;
    };
    # fbDOOM carries i_oplmusic.c but not the OPL emulator, MIDI reader, or
    # MUS converter it needs, and never compiles it. Those come from
    # Chocolate Doom 2.2.1, pinned rather than a later release because it is
    # the last one shipping DOSBox's dbopl emulator. Its successor, Nuked
    # OPL3, always emulates at 49716 Hz internally whatever output rate it is
    # asked for, which costs about 80% of a Deck core and cannot be tuned
    # down. dbopl runs at the rate it is given. opl.h is byte-identical
    # between the two releases, so the fork's music code is unaffected.
    chocolate-doom-src = {
      url = "github:chocolate-doom/chocolate-doom/2ddd290b8226e1d1f0c52e344e76150261a8c0c3";
      flake = false;
    };
    lua-src = {
      url = "https://www.lua.org/ftp/lua-5.5.0.tar.gz";
      flake = false;
    };
  };

  outputs =
    { self, nixpkgs, fenix, fceumm-src, gambatte-src, fuse-src, gpsp-src, lua-src
    , fbdoom-src, chocolate-doom-src }:
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
          ./native/doom
          ./native/src
          ./native/vendor/tamalib-rs
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
      gpspCore = pkgsCross.stdenv.mkDerivation {
        pname = "gpsp-core";
        version = "0.91-20260721-deck";
        src = gpsp-src;
        nativeBuildInputs = [ pkgs.gnumake ];
        buildInputs = [ pkgsCross.glibc.static ];
        NIX_CFLAGS_COMPILE = "-static -O3";
        NIX_LDFLAGS = "-static";
        buildPhase = ''
          runHook preBuild
          make -j$NIX_BUILD_CORES \
            platform=rpi2 \
            STATIC_LINKING=1 \
            TARGET=gpsp_libretro.a \
            CC=$CC \
            CXX=$CXX \
            AR=${pkgsCross.stdenv.cc.targetPrefix}ar
          runHook postBuild
        '';
        installPhase = ''
          runHook preInstall
          install -Dm644 gpsp_libretro.a $out/lib/libgpsp.a
          install -Dm644 COPYING \
            $out/share/licenses/gba-deck/gpSP-COPYING
          runHook postInstall
        '';
      };
      # Freedoom stands in for the owner's private IWAD in the sandboxed
      # frame-hash test. It is redistributable, so the test can be pinned and
      # reproduced by anyone; it is never installed on a Deck.
      freedoomWads = pkgs.fetchzip {
        url = "https://github.com/freedoom/freedoom/releases/download/v0.13.0/freedoom-0.13.0.zip";
        hash = "sha256-ieYfr4TYVRGUVriK/duN+iOlr8oAIAxz4IfnbG4hOis=";
      };

      # The license text fbDOOM's source headers point at. The fork ships no
      # COPYING of its own.
      gpl2Text = pkgs.fetchurl {
        url = "https://www.gnu.org/licenses/old-licenses/gpl-2.0.txt";
        hash = "sha256-7a72Msu2Q+TnoiFxemxEGkwafJGObk1W3rw9hzmyM/Y=";
      };

      # The DOOM engine as a static archive. Everything platform-specific is
      # dropped here and supplied by native/doom instead: the fork's
      # framebuffer backend would fight the BMC compositor for the panel, its
      # tty backend cannot see a THEGamepad, and its timer has to become
      # deterministic for the pinned frame hash.
      doomLib =
        let
          # The fork's own object list, minus the four files replaced by the
          # Retro Deck platform layer. i_main.c stays: the build renames its
          # main so myargc, myargv, and response-file handling are preserved.
          engineSources = [
            "i_main" "dummy" "am_map" "doomdef" "doomstat" "dstrings" "d_event"
            "d_items" "d_iwad" "d_loop" "d_main" "d_mode" "d_net" "f_finale"
            "f_wipe" "g_game" "hu_lib" "hu_stuff" "info" "i_cdmus" "i_endoom"
            "i_joystick" "i_scale" "i_sound" "i_system" "memio" "m_argv"
            "m_bbox" "m_cheat" "m_config" "m_controls" "m_fixed" "m_menu"
            "m_misc" "m_random" "p_ceilng" "p_doors" "p_enemy" "p_floor"
            "p_inter" "p_lights" "p_map" "p_maputl" "p_mobj" "p_plats"
            "p_pspr" "p_saveg" "p_setup" "p_sight" "p_spec" "p_switch"
            "p_telept" "p_tick" "p_user" "r_bsp" "r_data" "r_draw" "r_main"
            "r_plane" "r_segs" "r_sky" "r_things" "sha1" "sounds" "statdump"
            "st_lib" "st_stuff" "s_sound" "tables" "v_video" "wi_stuff"
            "w_checksum" "w_file" "w_file_stdc_unbuffered" "w_main" "w_wad"
            "z_zone" "aes_prng" "net_client" "net_common" "net_dedicated"
            "net_gui" "net_io" "net_loop" "net_packet" "net_petname"
            "net_query" "net_server" "net_structrw"
            # NOSDL selects the POSIX network driver.
            "net_posix"
          ];
          hostSources = [
            "i_video_retrodeck" "i_input_retrodeck" "i_timer_retrodeck"
            "i_sound_retrodeck" "opl_retrodeck"
          ];
          # Music: the fork's own i_oplmusic.c, paired with the Chocolate
          # Doom pieces it needs. opl_sdl.c is deliberately absent, replaced
          # by opl_retrodeck.c above; opl_linux.c and opl_obsd.c drive real
          # ISA hardware and have no meaning here.
          musicSources = [
            "i_oplmusic" "opl" "dbopl" "opl_queue" "midifile" "mus2mid"
          ];
        in
        pkgsCross.stdenv.mkDerivation {
          pname = "doom-lib";
          version = "0.1.0-20251201-deck";
          src = fbdoom-src;
          sourceRoot = "source/fbdoom";
          buildInputs = [ pkgsCross.glibc.static ];

          postPatch = ''
            # The frontend owns the real main.
            substituteInPlace i_main.c \
              --replace-fail 'int main(int argc, char **argv)' \
                'int doom_main(int argc, char **argv)'

            # The fork hardcodes its config and savegame directory to /mnt,
            # under a directory named after PACKAGE_TARNAME. The host points
            # it beside the WAD instead, which the deployer merges rather
            # than replaces, so savegames survive updates.
            substituteInPlace m_config.c \
              --replace-fail 'homedir = "/mnt";' \
                'homedir = getenv("RETRO_DECK_DOOM_DIR");
    if (homedir != NULL) {
      return M_StringJoin(homedir, DIR_SEPARATOR_S, NULL);
    }
    homedir = "/mnt";'

            # Register the Retro Deck sound module. Upstream compiles no
            # sound module at all without SDL, so the table is empty and
            # I_InitSound silently produces a mute game.
            substituteInPlace i_sound.c \
              --replace-fail 'extern sound_module_t sound_sdl_module;' \
                'extern sound_module_t sound_sdl_module;
extern sound_module_t sound_retrodeck_module;' \
              --replace-fail '#ifdef FEATURE_SOUND
    &sound_sdl_module,' \
                '    &sound_retrodeck_module,
#ifdef FEATURE_SOUND
    &sound_sdl_module,'

            # Register the OPL music module, which upstream also leaves out
            # of the table. i_oplmusic.c supplies opl_io_port itself.
            substituteInPlace i_sound.c \
              --replace-fail '#ifdef FEATURE_SOUND
    &music_sdl_module,' \
                '    &music_opl_module,
#ifdef FEATURE_SOUND
    &music_sdl_module,'

            # Vendor the OPL emulator, MIDI reader, and MUS converter.
            cp ${chocolate-doom-src}/opl/opl.c \
               ${chocolate-doom-src}/opl/opl.h \
               ${chocolate-doom-src}/opl/dbopl.c \
               ${chocolate-doom-src}/opl/dbopl.h \
               ${chocolate-doom-src}/opl/opl_internal.h \
               ${chocolate-doom-src}/opl/opl_queue.c \
               ${chocolate-doom-src}/opl/opl_queue.h \
               ${chocolate-doom-src}/src/midifile.c \
               ${chocolate-doom-src}/src/midifile.h \
               ${chocolate-doom-src}/src/mus2mid.c \
               ${chocolate-doom-src}/src/mus2mid.h \
               .
            chmod u+w opl.c dbopl.c opl_queue.c midifile.c mus2mid.c

            # midifile.c reads big-endian MIDI headers through SDL's byte
            # swappers. fbDOOM's i_swap.h only covers the little-endian
            # direction, so supply the two the reader needs.
            substituteInPlace midifile.c \
              --replace-fail '#include "midifile.h"' '#include "midifile.h"

#if defined(__BYTE_ORDER__) && __BYTE_ORDER__ == __ORDER_BIG_ENDIAN__
#define SDL_SwapBE16(x) ((uint16_t) (x))
#define SDL_SwapBE32(x) ((uint32_t) (x))
#else
#define SDL_SwapBE16(x) __builtin_bswap16((uint16_t) (x))
#define SDL_SwapBE32(x) __builtin_bswap32((uint32_t) (x))
#endif'

            # Chocolate Doom's OPL dispatcher lists hardware drivers and the
            # SDL one; only the Retro Deck software driver survives here.
            substituteInPlace opl.c \
              --replace-fail 'extern opl_driver_t opl_sdl_driver;' \
                'extern opl_driver_t opl_retrodeck_driver;' \
              --replace-fail '    &opl_sdl_driver,' \
                '    &opl_retrodeck_driver,' \
              --replace-fail '#include "SDL.h"' '#include <string.h>'

            # OPL_Delay blocked on a condition variable that the SDL audio
            # thread signalled; it is the only reason opl.c wanted SDL. Chip
            # detection depends on it advancing emulated time, so it maps
            # onto the driver's render-and-discard instead of becoming a
            # no-op.
            ${pkgs.python3}/bin/python3 - <<'PATCH'
source = open("opl.c").read()
# Replace the whole condition-variable delay block: the data struct, its
# callback, and OPL_Delay itself.
start = source.index("typedef struct\n{\n    int finished;")
end = source.index("void OPL_SetPaused(int paused)")
source = source[:start] + """extern void retrodeck_opl_delay(uint64_t microseconds);

void OPL_Delay(uint64_t us)
{
    if (driver != NULL)
    {
        retrodeck_opl_delay(us);
    }
}

""" + source[end:]
open("opl.c", "w").write(source)
PATCH

            cp ${./native/doom}/*.c ${./native/doom}/*.h .
          '';

          buildPhase = ''
            runHook preBuild
            # -O2 with Cortex-A7 tuning rather than the -Ofast used for the
            # emulator cores: -ffast-math would change DOOM's arithmetic and
            # desynchronise the demos the frame-hash test depends on.
            flags="-c -std=gnu99 -O2 -marm -march=armv7-a -mtune=cortex-a7 \
              -mfpu=neon-vfpv4 -mfloat-abi=hard -fomit-frame-pointer \
              -DNORMALUNIX -DLINUX -DSNDSERV -DFEATURE_MULTIPLAYER -I."
            for unit in ${pkgs.lib.concatStringsSep " "
                (engineSources ++ hostSources ++ musicSources)}; do
              echo "[compiling $unit]"
              $CC $flags -o "$unit.o" "$unit.c"
            done
            ${pkgsCross.stdenv.cc.targetPrefix}ar rcs libdoom.a *.o
            runHook postBuild
          '';

          installPhase = ''
            runHook preInstall
            install -Dm644 libdoom.a $out/lib/libdoom.a
            install -Dm644 ../README.TXT \
              $out/share/licenses/doom-deck/fbDOOM-README.TXT
            # Neither fbDOOM nor the Deck fork ships a license file, so the
            # GPL-2.0 text their headers refer to is pinned by hash here.
            install -Dm644 ${gpl2Text} \
              $out/share/licenses/doom-deck/DOOM-COPYING
            # The OPL emulator, MIDI reader, and MUS converter.
            install -Dm644 ${chocolate-doom-src}/COPYING \
              $out/share/licenses/doom-deck/Chocolate-Doom-COPYING
            runHook postInstall
          '';

          meta = {
            description = "fbDOOM engine archive with the Retro Deck platform layer";
            license = pkgs.lib.licenses.gpl2Only;
          };
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
            # Scoped to one binary on purpose: retro-host and doom-host each
            # need a statically linked engine that this derivation does not
            # provide, so an unscoped build would try to link them and fail.
            cargo build --release --locked --offline --features lisp-image \
              --bin retrodeck-native
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

        tamagotchi-deck = pkgs.stdenvNoCC.mkDerivation {
          pname = "tamagotchi-deck";
          version = "0.1.0";
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
              -L native=${gmeStaticCross}/lib \
              -L native=${pkgsCross.glibc.static}/lib \
              -l m"
            cargo build --release --locked --offline --bin tamagotchi-host
            runHook postBuild
          '';

          installPhase = ''
            runHook preInstall
            install -Dm755 target/armv7-unknown-linux-gnueabihf/release/tamagotchi-host \
              $out/bin/tamagotchi-deck
            nuke-refs $out/bin/tamagotchi-deck
            runHook postInstall
          '';

          meta = {
            description = "Tamagotchi P1 host for the Braiins Forge Deck";
            license = pkgs.lib.licenses.mit;
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

        doom-deck = pkgs.stdenvNoCC.mkDerivation {
          pname = "doom-deck";
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
            doomLib
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
              -L native=${doomLib}/lib \
              -L native=${pkgsCross.glibc.static}/lib \
              -l static=doom \
              -l z \
              -l m"
            cargo build --release --locked --offline --bin doom-host
            runHook postBuild
          '';

          installPhase = ''
            runHook preInstall
            install -Dm755 \
              target/armv7-unknown-linux-gnueabihf/release/doom-host \
              $out/bin/doom-deck
            mkdir -p $out/share/licenses
            cp -a ${doomLib}/share/licenses/doom-deck \
              $out/share/licenses/doom-deck
            nuke-refs $out/bin/doom-deck
            runHook postInstall
          '';

          meta = {
            description = "fbDOOM engine with the Rust Deck frontend";
            license = pkgs.lib.licenses.gpl2Only;
            platforms = [ system ];
          };
        };

        gba-deck = retroHost {
          name = "gba-deck";
          core = "gba";
          coreDerivation = gpspCore;
          coreLibrary = "gpsp";
          description = "gpSP GBA core with the Rust Deck frontend";
          license = pkgs.lib.licenses.gpl2Only;
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
            # The standard variant carries json/slice/bytearray for the
            # deploy-time scene installer; the minimal one strips Python
            # down past what any real script can use.
            make -C ports/unix -j$NIX_BUILD_CORES \
              VARIANT=standard \
              FROZEN_MANIFEST= \
              MICROPY_PY_FFI=0 \
              MICROPY_USE_READLINE=0 \
              MICROPY_PY_TERMIOS=0 \
              MICROPY_PY_SOCKET=0 \
              MICROPY_PY_SSL=0 \
              MICROPY_PY_THREAD=0 \
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
            install -m755 ports/unix/build-standard/micropython \
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
          # GPL homebrew fetched by hash; runs on gpSP's built-in HLE BIOS.
          run ${self.packages.${system}.gba-deck}/bin/gba-deck \
            ${pkgs.fetchurl {
              url = "https://github.com/pinobatch/240p-test-mini/releases/download/v0.23/240pee_mb.gba";
              hash = "sha256-R4RPcUBzigb487wJeA2jqwlVOaJQuHD2FP7tVh2dbzQ=";
            }} 408bb5792cfa3e00
          touch $out
        '';

        # DOOM's own frame-hash check. Freedoom stands in for the owner's
        # private IWAD: it is redistributable, so the sandbox can pin a
        # hash. Determinism comes from the host's test-mode clock, which is
        # derived from the presented frame count rather than the machine.
        doom-host-smoke = pkgs.runCommand "doom-host-smoke" { } ''
          cp ${freedoomWads}/freedoom1.wad freedoom1.wad
          chmod 600 freedoom1.wad
          # 600 frames rather than the emulators' 120: the title screen has
          # to give way to demo playback before any weapon fires, and the
          # sound effects are only covered once one does.
          # Music is off by default on a Deck because the OPL emulation
          # cannot hold the frame budget; the test turns it on explicitly so
          # the code path stays covered.
          RETRO_DECK_TEST_FRAMES=600 RETRO_DECK_VOLUME_PERCENT=0 \
            RETRO_DECK_DOOM_MUSIC=1 \
            ${pkgs.qemu-user}/bin/qemu-arm \
            ${self.packages.${system}.doom-deck}/bin/doom-deck \
            freedoom1.wad > run.log 2>&1
          cat run.log
          grep -q "test frames=600 video=600 " run.log
          # Pinned together on purpose: if the frame hash ever moves, the
          # sleep count and synthetic clock say whether the engine's timing
          # changed or only its rendering did.
          grep -q "sleeps=109 ms=16909 hash=a8cf511b608229c1" run.log
          # The engine must have reached actual gameplay setup, not just
          # printed a banner before dying.
          grep -q "ST_Init: Init status bar" run.log
          # The mixer must have parsed real DMX lumps and produced audio,
          # not quietly emitted silence.
          grep -q "sound effects ready at 44100 Hz" run.log
          grep -q "audio=1452640 sfx=87 " run.log
          # Music has to actually play, which is three separate claims: the
          # chip-detection handshake accepted the driver, the music module
          # drove it with register writes and scheduled MIDI callbacks, and
          # the result carried signal rather than silence. Asserting only the
          # last of those once passed while nothing played at all, because an
          # uninitialised chip emitted a little noise.
          grep -q "OPL music ready, chip at 44100 Hz" run.log
          grep -q "OPL_Init: Using driver 'Retrodeck'" run.log
          grep -Eq "music=[0-9]{6,}/[0-9]{4,}/[0-9]{3,} " run.log
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
