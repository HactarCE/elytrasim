#!/bin/bash
# One explorer cell: "<speed> <pitch>" in, $RUN/cells/v<speed>_p<pitch>.csv out.
set -u
read -r v g <<< "$1"
exec "$HOME/booster-gain/target/release/examples/booster" explore "$v" "$g" "$RUN/cells"
