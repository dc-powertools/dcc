#!/bin/sh
set -eu

# No network access or external installers: changing this option creates an image
# upgrade whose payload and defaults are independently observable.
case "${IMAGEVERSION:-v1}" in
    v1|v2) image_version=${IMAGEVERSION:-v1} ;;
    *) printf '%s\n' 'unsupported fixture image version' >&2; exit 1 ;;
esac

fixture_user=${_REMOTE_USER:-fixture}
fixture_home=${_REMOTE_USER_HOME:-/home/fixture}
fixture_root=$fixture_home/.persistence-fixture

mkdir -p "$fixture_root/packages/bin" "$fixture_root/scratch" \
    "$fixture_root/sessions" "$fixture_root/cache" "$fixture_root/preferences"
printf '%s\n' "$image_version" > "$fixture_root/packages/release"
printf '#!/bin/sh\nprintf "%%s\\n" "%s"\n' "$image_version" \
    > "$fixture_root/packages/bin/fixture-tool"
chmod 0755 "$fixture_root/packages/bin/fixture-tool"
printf 'image-default-%s\n' "$image_version" > "$fixture_root/preferences/settings"
printf 'image-session-%s\n' "$image_version" > "$fixture_root/sessions/initial"
printf 'image-cache-%s\n' "$image_version" > "$fixture_root/cache/initial"
chown -R "$fixture_user:$(id -gn "$fixture_user")" "$fixture_root"

# Relocate in the installation layer, so payload is never copied out of a lower
# image layer. Scratch uses precisely the same contract as installed packages.
mkdir -p /opt/dcc-local/persistence
mv "$fixture_root/packages" /opt/dcc-local/persistence/packages
mv "$fixture_root/scratch" /opt/dcc-local/persistence/scratch
ln -sf /opt/dcc-local/persistence/packages "$fixture_root/packages"
ln -sf /opt/dcc-local/persistence/scratch "$fixture_root/scratch"

mkdir -p /usr/local/libexec
cp ./assert-ready.sh /usr/local/libexec/persistence-fixture-assert-ready
chmod 0755 /usr/local/libexec/persistence-fixture-assert-ready
