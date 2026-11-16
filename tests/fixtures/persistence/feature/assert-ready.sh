#!/bin/sh
set -eu

# Register this helper as both a build-preparation and startup hook in tests. It
# proves the persistence links exist before user hooks and are user-writable.
fixture_root=${1:-$HOME/.persistence-fixture}
test -L "$fixture_root/packages"
test -L "$fixture_root/scratch"
test -d "$fixture_root/packages"
test -d "$fixture_root/scratch"
test -r "$fixture_root/preferences/settings"
test -x "$fixture_root/packages/bin/fixture-tool"
package_probe=$(mktemp "$fixture_root/packages/.hook-writable.XXXXXX")
scratch_probe=$(mktemp "$fixture_root/scratch/.hook-writable.XXXXXX")
rm "$package_probe" "$scratch_probe"
printf '%s\n' ready
