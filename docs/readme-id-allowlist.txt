# Dotted tokens the READMEs print that are not registry ids, so `check-readme-ids.sh` does
# not send a reader to look them up.
#
# The extraction takes any lowercase word with a dot in it, because the pattern that would
# separate an id from an attribute chain by shape does not exist: `array.array` and
# `bwepoch.fixed_window` are the same shape. So every exclusion is written down here, where
# a reviewer sees it, and the script fails when an entry stops appearing in either README
# rather than letting this file grow into a way of hiding a real id.

# Python and NumPy attribute chains from the worked examples.
array.array
force.astype
np.float32
np.loadtxt
pf.analyse_countermovement_jump
pf.partition_sentinel_values
jump.jump_height_takeoff_frame_meters
jump.jump_height_takeoff_frame_meters.describe
jump.jump_height_takeoff_frame_meters.value
jump.time_to_takeoff_seconds.provenance.parameters_of
jump.unregistered_methods
jump.weighing_epoch_tied_window_count
provenance.acquisition_complete
registry.method
registry.methods_that_can_fail
trial.exclusions

# Fields on a raised exception, shown so a caller knows what it can branch on.
error.method_id
error.parameter
error.value

# A field on a registry entry, not an entry.
gui.surfacing

# Arguments to the terminal's --set flag, which names a slot and a parameter on that
# slot's bound rule rather than naming an entry.
onset.k
takeoff.threshold_n
weighing.duration

# Filenames and two hostnames.
headline_audit.py
trial.csv
install.md
dralexharrison.github.io
github.com
