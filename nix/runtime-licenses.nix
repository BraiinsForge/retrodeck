{
  pkgs,
  pkgsCross,
  staticCross,
  nixpkgsSource,
  nativeCargoDeps,
}:

let
  wayland = staticCross.wayland;
  libpng = staticCross.libpng;
  zlib = staticCross.zlib;
  libffi = staticCross.libffi;
  glibc = pkgsCross.glibc;
  libvorbis = pkgs.libvorbis;
  libogg = pkgs.libogg;
  gme = pkgs.game-music-emu;
in
pkgs.runCommand "retro-deck-runtime-licenses" {
  allowedReferences = [ ];
  nativeBuildInputs = [
    pkgs.gnutar
    pkgs.gzip
    pkgs.xz
  ];

  meta.description = "License notices for shared Retro Deck dependencies";
} ''
  licenses=$out/share/licenses/runtime
  mkdir -p "$licenses"

  tar -xOf ${wayland.src} wayland-${wayland.version}/COPYING \
    > "$licenses/Wayland-COPYING"
  rust_notices="$licenses/Rust-crates-NOTICES.txt"
  printf '%s\n' 'Rust dependency notices for retrodeck-native' > "$rust_notices"
  append_rust_notice() {
    crate=$1
    file=$2
    printf '\n===== %s/%s =====\n\n' "$crate" "$file" >> "$rust_notices"
    cat "${nativeCargoDeps}/$crate/$file" >> "$rust_notices"
  }
  append_rust_notice aho-corasick-1.1.5 LICENSE-MIT
  append_rust_notice arrayref-0.3.9 LICENSE
  append_rust_notice arrayvec-0.7.8 LICENSE-MIT
  append_rust_notice base64-0.22.1 LICENSE-APACHE
  append_rust_notice base64-0.22.1 LICENSE-MIT
  append_rust_notice bitflags-2.13.1 LICENSE-MIT
  append_rust_notice block-buffer-0.10.4 LICENSE-APACHE
  append_rust_notice block-buffer-0.10.4 LICENSE-MIT
  append_rust_notice bitvec-1.1.1 LICENSE.txt
  append_rust_notice bytemuck-1.25.2 LICENSE-MIT
  append_rust_notice cfg-if-1.0.4 LICENSE-MIT
  append_rust_notice cfg_aliases-0.2.2 LICENSE
  append_rust_notice cpufeatures-0.2.17 LICENSE-APACHE
  append_rust_notice cpufeatures-0.2.17 LICENSE-MIT
  append_rust_notice crypto-common-0.1.7 LICENSE-APACHE
  append_rust_notice crypto-common-0.1.7 LICENSE-MIT
  append_rust_notice digest-0.10.7 LICENSE-APACHE
  append_rust_notice digest-0.10.7 LICENSE-MIT
  append_rust_notice downcast-rs-1.2.1 LICENSE-MIT
  append_rust_notice evdev-0.13.2 LICENSE-APACHE
  append_rust_notice evdev-0.13.2 LICENSE-MIT
  append_rust_notice funty-2.0.0 LICENSE.txt
  append_rust_notice generic-array-0.14.7 LICENSE
  append_rust_notice hmac-0.12.1 LICENSE-APACHE
  append_rust_notice hmac-0.12.1 LICENSE-MIT
  append_rust_notice linux-raw-sys-0.12.1 LICENSE-MIT
  append_rust_notice log-0.4.33 LICENSE-MIT
  append_rust_notice memchr-2.8.3 LICENSE-MIT
  append_rust_notice nix-0.29.0 LICENSE
  append_rust_notice pbkdf2-0.12.2 LICENSE-APACHE
  append_rust_notice pbkdf2-0.12.2 LICENSE-MIT
  append_rust_notice proc-macro2-1.0.107 LICENSE-MIT
  append_rust_notice quick-xml-0.39.4 LICENSE-MIT.md
  append_rust_notice quote-1.0.47 LICENSE-MIT
  append_rust_notice radium-0.7.0 LICENSE.txt
  append_rust_notice regex-1.13.1 LICENSE-APACHE
  append_rust_notice regex-1.13.1 LICENSE-MIT
  append_rust_notice regex-automata-0.4.18 LICENSE-APACHE
  append_rust_notice regex-automata-0.4.18 LICENSE-MIT
  append_rust_notice regex-syntax-0.8.11 LICENSE-APACHE
  append_rust_notice regex-syntax-0.8.11 LICENSE-MIT
  append_rust_notice rustix-1.1.4 LICENSE-MIT
  append_rust_notice sha2-0.10.9 LICENSE-APACHE
  append_rust_notice sha2-0.10.9 LICENSE-MIT
  append_rust_notice smallvec-1.15.2 LICENSE-MIT
  append_rust_notice strict-num-0.1.1 LICENSE
  append_rust_notice subtle-2.6.1 LICENSE
  append_rust_notice tap-1.0.1 LICENSE.txt
  append_rust_notice tiny-skia-0.12.0 LICENSE
  append_rust_notice tiny-skia-path-0.12.0 LICENSE
  append_rust_notice typenum-1.20.1 LICENSE-APACHE
  append_rust_notice typenum-1.20.1 LICENSE-MIT
  append_rust_notice unicode-ident-1.0.24 LICENSE-MIT
  append_rust_notice unicode-ident-1.0.24 LICENSE-UNICODE
  append_rust_notice version_check-0.9.5 LICENSE-APACHE
  append_rust_notice version_check-0.9.5 LICENSE-MIT
  append_rust_notice wayland-backend-0.3.15 LICENSE.txt
  append_rust_notice wayland-client-0.31.14 LICENSE.txt
  append_rust_notice wayland-scanner-0.31.10 LICENSE.txt
  append_rust_notice wayland-sys-0.31.11 LICENSE.txt
  append_rust_notice wyz-0.5.1 LICENSE.txt

  tar -xOf ${libpng.src} libpng-${libpng.version}/LICENSE \
    > "$licenses/libpng-LICENSE"
  tar -xOf ${zlib.src} zlib-${zlib.version}/LICENSE \
    > "$licenses/zlib-LICENSE"
  tar -xOf ${libffi.src} libffi-${libffi.version}/LICENSE \
    > "$licenses/libffi-LICENSE"
  tar -xOf ${glibc.src} glibc-${glibc.version}/COPYING \
    > "$licenses/glibc-COPYING"
  tar -xOf ${glibc.src} glibc-${glibc.version}/COPYING.LIB \
    > "$licenses/glibc-COPYING.LIB"
  tar -xOf ${libvorbis.src} libvorbis-${libvorbis.version}/COPYING \
    > "$licenses/libvorbis-COPYING"
  tar -xOf ${libogg.src} libogg-${libogg.version}/COPYING \
    > "$licenses/libogg-COPYING"
  tar -xOf ${gme.src} game-music-emu-${gme.version}/license.txt \
    > "$licenses/game-music-emu-LICENSE"
  install -m444 ${nixpkgsSource}/COPYING "$licenses/Nixpkgs-COPYING"
  install -m444 ${../native/vendor/tamalib-rs/LICENSE} \
    "$licenses/tamalib-rs-MIT"
  install -m444 ${../assets/settings-cog/UPSTREAM.txt} \
    "$licenses/knekko-CC0-NOTICE.txt"
  install -m444 ${../deploy/menu/ASSETS.md} \
    "$licenses/menu-assets-provenance.md"
  install -m444 ${../chiptunes/README.md} \
    "$licenses/chiptunes-CC0-provenance.md"
''
