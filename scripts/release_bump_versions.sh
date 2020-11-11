#!/bin/bash

set -xeuo pipefail

if [[ $# -ne 3 ]]; then
    echo "usage: next_version next_flask_version next_django_version"
    exit 1
fi

NEXT_VERSION=$1
NEXT_VERSION_FLASK=$2
NEXT_VERSION_DJANGO=$3

SCRIPT=$(realpath "$0")
BASE_DIR=$(dirname $(dirname "$SCRIPT"))

echo "Updating versions to $1"

function _sed {
    COMMAND="$1"
    FILE="$2"
    # TODO make sed fail of the replacement fails.
    sed -E -i '' -e "$COMMAND" "$FILE"
}

function update_python_version {
    VERSION=$1
    FILE=$2

    _sed "s/__version__ = \"[0-9\.]+\"/__version__ = \"$VERSION\"/" $FILE
}

update_python_version "$NEXT_VERSION" "$BASE_DIR/languages/python/oso/oso/oso.py"
update_python_version "$NEXT_VERSION_FLASK" "$BASE_DIR/languages/python/flask-oso/flask_oso/__init__.py"
update_python_version "$NEXT_VERSION_DJANGO" "$BASE_DIR/languages/python/django-oso/django_oso/__init__.py"
