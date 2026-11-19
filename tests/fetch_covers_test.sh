#!/bin/sh

set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
fetcher=$repo_root/deploy/menu/fetch-covers

temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT HUP INT TERM

cat > "$temporary/nes.tsv" <<'EOF'
tetris-tengen.png	Tetris (USA) (Tengen) (Unl).png
tetris-world.png	Tetris (World) (Collection).png
tetris-usa.png	Tetris (USA).png
kirby-rev1.png	Kirby's Adventure (USA) (Rev 1).png
kirby-usa.png	Kirby's Adventure (USA).png
EOF

cat > "$temporary/gb.tsv" <<'EOF'
ffl-collection.png	Final Fantasy Legend III (World) (Collection of SaGa).png
ffl-usa.png	Final Fantasy Legend III (USA).png
EOF

cat > "$temporary/zx.tsv" <<'EOF'
elite-alt.png	Elite (Europe) (Alt 1).png
elite-europe.png	Elite (Europe).png
knight-lore.png	Knight Lore (Europe).png
EOF

cat > "$temporary/index.html" <<'EOF'
<a href="Kirby%27s%20Adventure%20%28USA%29.png">Kirby's Adventure (USA).png</a>
<a href="Tetris%20%28World%29.png">Tetris (World).png</a>
<a href="nested/Rejected.png">nested path</a>
EOF

$fetcher --decode-index "$temporary/index.html" "$temporary/decoded.tsv"
cat > "$temporary/expected-decoded.tsv" <<'EOF'
Kirby%27s%20Adventure%20%28USA%29.png	Kirby's Adventure (USA).png
Tetris%20%28World%29.png	Tetris (World).png
EOF
cmp "$temporary/expected-decoded.tsv" "$temporary/decoded.tsv"

assert_match() {
	title=$1
	index=$2
	expected=$3
	actual=$($fetcher --best-match "$title" "$index")
	case $actual in
		"$expected"*) ;;
		*)
			echo "$title matched $actual instead of $expected" >&2
			exit 1
			;;
	esac
}

assert_match 'Tetris' "$temporary/nes.tsv" 'tetris-usa.png'
assert_match "Kirby's Adventure" "$temporary/nes.tsv" 'kirby-usa.png'
assert_match 'Final Fantasy Legend III' "$temporary/gb.tsv" 'ffl-usa.png'
assert_match 'Elite' "$temporary/zx.tsv" 'elite-europe.png'
assert_match 'Knight Lore' "$temporary/zx.tsv" 'knight-lore.png'

# Supply a cached SNES index, so checking system routing and title matching
# needs neither Internet access nor an actual ROM or PNG.
covers=$temporary/covers
mkdir -p "$covers/.libretro-index"
cat > "$covers/.libretro-index/snes-Named_Boxarts.tsv" <<'EOF'
all-stars.png	Super Mario All-Stars (USA).png
all-stars-world.png	Super Mario All-Stars + Super Mario World (USA).png
EOF
printf 'snes-test\tSuper Mario All-Stars + Super Mario World\tsnes\t/not-used.sfc\t#ffaf87\n' > "$temporary/games.tsv"
match_log=$(
	wget() { return 1; }
	set -- "$temporary/games.tsv" "$covers"
	. "$fetcher"
)
case $match_log in
	*'snes-test <- Named_Boxarts/Super Mario All-Stars + Super Mario World (USA).png'*) ;;
	*) echo 'SNES box art was not resolved' >&2; exit 1 ;;
esac
echo "fetch-covers-test: OK"
