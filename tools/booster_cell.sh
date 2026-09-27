#!/bin/bash
# One apex cell: "<speed> <pitch>" in, one line "speed,pitch,apex,apex_tick" to $RUN/cells/.
set -u
read -r v g <<< "$1"
"$HOME/booster-gain/target/release/examples/booster" cell "$v" "$g" > "$RUN/cells/v${v}_p${g}.txt"
