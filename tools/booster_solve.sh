#!/bin/bash
# One explorer cell: "<speed> <angle>" in, $RUN/cells/v<speed>_g<angle>.csv out.
set -u
read -r v g <<< "$1"
exec "$HOME/booster-gain/target/release/examples/booster" explore "$v" "$g" "$RUN/cells"
