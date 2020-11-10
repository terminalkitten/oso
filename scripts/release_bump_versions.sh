#!/bin/bash

set -xeuo pipefail

NEXT_VERSION=$1

SCRIPT=$(realpath "$0")
BASE_DIR=$(dirname $(dirname "$SCRIPT"))

echo "Updating versions to $1"

function _sed {
    COMMAND="$1"
    FILE="$2"
    sed -E -i '' -e "$COMMAND ; t ; q1 ;" "$FILE"
}

function update_python_version {
    VERSION=$1
    FILE=$2

    _sed "s/__version__ = \"[0-9\.]+\"/__version__ = \"$VERSION\"/" $FILE
}

update_python_version "$NEXT_VERSION" "$BASE_DIR/languages/python/oso/oso/oso.py"
