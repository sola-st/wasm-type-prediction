#!/bin/bash
# kill all spawned processes e.g., on CTRL C
# see https://linuxconfig.org/how-to-propagate-a-signal-to-child-processes-from-a-bash-script
trap 'echo killing all background jobs...; kill $(jobs -p)' INT

source ~/emsdk/emsdk_env.sh 

for n in {1..16}
do
    echo "spawning worker $n..."
    ./sources-compile.py --shuffle --logfile "log-$n.txt" >/dev/null &
    
    # start each subsequent worker a little bit later to avoid race conditions on the first directory
    sleep 1
done

# list all parallel workers for an overview
jobs -l

# wait for all background jobs to finish...
wait
