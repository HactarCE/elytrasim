#!/bin/bash
# Bring a finished sweep home and delete it from the cluster.
#
#   tools/snapsweep_pull.sh <remote-run-dir> <dest> [--keep]
#
# e.g. tools/snapsweep_pull.sh mapfine runs/atlas/mapfine
#
# The cluster is compute only. Leaving results there splits the corpus in two and you lose
# track of which copy is authoritative, so the pull deletes the remote run directory as its
# last act -- per sweep, not per session. `--keep` opts out for a sweep you intend to resume.
#
# Only the run directory goes. The toolchain, the source tree and the compiled target/ are not
# data and are left alone: the filesystem there is 2.3T at 6% used, so deleting anything
# rebuildable buys nothing and costs a cold compile on the next run.
#
# Deletion is gated on a name-by-name comparison, not on a count: the point of the check is to
# catch a truncated transfer, and two different sets can have the same size. Nothing is removed
# unless every remote cell is present locally and every cell holds the profile count the remote
# reported for it.
#
# Tarred on the far side rather than rsynced file-by-file: ~20k profiles of a few kB each is a
# case where per-file round trips cost far more than the bytes do (13s for 176MB).
set -u
REMOTE=${1:?usage: snapsweep_pull.sh <remote-run-dir> <dest> [--keep]}
DEST=${2:?usage: snapsweep_pull.sh <remote-run-dir> <dest> [--keep]}
KEEP=${3:-}
HOST=${HOST:-cif-cpu}
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$DEST" || exit 1

echo ">> manifest from $HOST:~/$REMOTE"
ssh -o BatchMode=yes "$HOST" "bash -s" <<EOF > "$TMP/remote.txt" || exit 1
cd ~/$REMOTE || exit 1
for d in out/*/; do c=\$(basename "\$d"); echo "\$c \$(ls "\$d"tight_t*.pitches 2>/dev/null | wc -l)"; done
EOF
[ -s "$TMP/remote.txt" ] || { echo "remote manifest is empty -- refusing to continue" >&2; exit 1; }
echo "   $(wc -l < "$TMP/remote.txt" | tr -d ' ') cells remote"

echo ">> packing and unpacking into $DEST"
ssh -o BatchMode=yes "$HOST" "cd ~/$REMOTE && tar czf - cells.tsv out" | tar xzf - -C "$DEST" || exit 1

echo ">> verifying every remote cell arrived intact"
bad=0
while read -r cell n; do
  got=$(ls "$DEST/out/$cell"/tight_t*.pitches 2>/dev/null | wc -l | tr -d ' ')
  if [ "$got" != "$n" ]; then echo "   MISMATCH $cell: remote $n, local $got" >&2; bad=$((bad+1)); fi
done < "$TMP/remote.txt"
cells=$(wc -l < "$TMP/remote.txt" | tr -d ' ')
prof=$(find "$DEST/out" -name 'tight_t*.pitches' | wc -l | tr -d ' ')
if [ "$bad" != 0 ]; then
  echo "$bad cell(s) did not arrive intact -- remote KEPT at $HOST:~/$REMOTE" >&2
  exit 1
fi
echo "   $cells/$cells cells intact, $prof profiles -> $DEST"

if [ "$KEEP" = "--keep" ]; then
  echo ">> --keep given; leaving $HOST:~/$REMOTE in place"
else
  sz=$(ssh -o BatchMode=yes "$HOST" "du -sh ~/$REMOTE 2>/dev/null | cut -f1")
  ssh -o BatchMode=yes "$HOST" "rm -rf ~/${REMOTE:?}" || exit 1
  echo ">> deleted $HOST:~/$REMOTE ($sz freed)"
fi
