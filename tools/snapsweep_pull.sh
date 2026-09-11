#!/bin/bash
# Bring the finished cells home. The cluster is compute-only; the corpus lives here.
#
#   tools/snapsweep_pull.sh [dest]      default runs/atlas/snapsweep
#
# Tarred on the far side rather than rsynced file-by-file: ~20k profiles of a few kB each is a
# case where per-file round trips cost more than the bytes do.
set -u
SELF=$(cd "$(dirname "$0")" && pwd)
DEST=${1:-$SELF/../runs/atlas/snapsweep}
HOST=${HOST:-cif-cpu}
mkdir -p "$DEST" || exit 1
echo ">> packing on $HOST"
ssh -o BatchMode=yes "$HOST" 'cd ~/atlas-run && tar czf - cells.tsv out' > "$DEST/.snapsweep.tar.gz" || exit 1
echo ">> unpacking into $DEST"
tar xzf "$DEST/.snapsweep.tar.gz" -C "$DEST" --strip-components=0 || exit 1
rm -f "$DEST/.snapsweep.tar.gz"
cells=$(find "$DEST/out" -maxdepth 1 -mindepth 1 -type d | wc -l | tr -d ' ')
prof=$(find "$DEST/out" -name 'tight_t*.pitches' | wc -l | tr -d ' ')
echo "$cells cells, $prof profiles -> $DEST"
