#!/usr/bin/env bash

watch -n 2 '
  TARGET_PID=$(ps axuw | grep "qateel -c ./config.toml" | grep -v grep | awk "{print \$2}");
  if [ -z "$TARGET_PID" ]; then
    echo "Process not found!";
  else
    # Dynamically find the PID and all recursive child PIDs
    PIDS=$(flatten_pids() { echo "$1"; for c in $(pgrep -P "$1"); do flatten_pids "$c"; done; }; flatten_pids "$TARGET_PID" | tr "\n" ",");
    PIDS=${PIDS%,};

    echo "Monitoring Parent PID: $TARGET_PID (Total Procs: $(echo $PIDS | tr "," "\n" | wc -l))";
    echo "--------------------------------------------------";

    # Extract CPU and RSS (KB) for all found PIDs, then sum them up
    ps -p "$PIDS" -o %cpu,rss= 2>/dev/null | awk '\''
      {cpu+=$1; rss+=$2}
      END {printf "Total CPU Usage: %.2f%%\nTotal RAM Usage: %.2f MB\n", cpu, rss/1024}
    '\'';
  fi
'
