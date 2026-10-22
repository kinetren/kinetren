#!/usr/bin/env bash
# Updating third-party .otio samples for cross-render conformance (test 158, R.25).
#
# Files ARE COMMITTED (owner’s decision R.25 item 2: “from the internet” is about provenance,
# not about the delivery method; the test must work offline).
# The script is only needed to raise the pin and see drift via git diff.
set -euo pipefail

# Pin of the upstream tests/sample_data directory (Apache-2.0).
PIN=73803eff1dbf86b82bcf97179567a243f4036b90
RAW=https://raw.githubusercontent.com/AcademySoftwareFoundation/OpenTimelineIO/$PIN

dir="$(cd "$(dirname "$0")/../fixtures/otio-samples" && pwd)"
cd "$dir"

get() { # get <relative path in the repo> <file name on our side>
    curl -fsSL "$RAW/$1" -o ".tmp.$2"
    mv ".tmp.$2" "$2"
}

for f in simple_cut multiple_track multitrack preflattened transition_test \
         screening_example nested_example premiere_example \
         generator_reference_test; do
    get "tests/sample_data/$f.otio" "$f.otio"
    echo "  $f.otio"
done
get LICENSE.txt LICENSE

echo "pin: $PIN (when updating, fix README.md too)"
