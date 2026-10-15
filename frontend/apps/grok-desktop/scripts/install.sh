#!/bin/bash
# Desktop installer. Re-exec the tracked CLI installer. The only SHA-256 pin
# body stays in crates/codegen/xai-grok-pager/scripts/install.sh.
# Tests clear the environment and prepend a fake curl directory to PATH.
# Do not look up bash or dirname on that PATH. dirname is not a shell builtin,
# and Nix often has no /bin/bash. The tests start this file with bash, so the
# interpreter already running is $BASH. Direct kernel exec still uses the
# shebang; the tests do not do that.
set -eu

case $0 in
  */*) here=${0%/*} ;;
  *) here=. ;;
esac
if [ -z "$here" ]; then
  here=/
fi

installer=$here/../../../../crates/codegen/xai-grok-pager/scripts/install.sh

if [ ! -f "$installer" ]; then
  dir=$here
  installer=
  while [ -n "$dir" ] && [ "$dir" != / ]; do
    candidate=$dir/crates/codegen/xai-grok-pager/scripts/install.sh
    if [ -f "$candidate" ]; then
      installer=$candidate
      break
    fi
    case $dir in
      */*) dir=${dir%/*} ;;
      *)
        if [ -f ./crates/codegen/xai-grok-pager/scripts/install.sh ]; then
          installer=./crates/codegen/xai-grok-pager/scripts/install.sh
        fi
        break
        ;;
    esac
    if [ -z "$dir" ]; then
      dir=/
    fi
  done
fi

if [ -z "${installer:-}" ] || [ ! -f "$installer" ]; then
  echo "desktop installer: tracked CLI installer not found from $0" >&2
  exit 1
fi

# $BASH is unset when sh starts this file. Do not search PATH for the name bash.
interp=
if [ -n "${BASH:-}" ] && [ -x "${BASH}" ]; then
  interp=$BASH
elif [ -x /bin/bash ]; then
  interp=/bin/bash
elif [ -x /usr/bin/bash ]; then
  interp=/usr/bin/bash
fi

if [ -z "$interp" ]; then
  echo "desktop installer: no bash interpreter without a PATH lookup" >&2
  exit 127
fi

exec "$interp" "$installer" "$@"
