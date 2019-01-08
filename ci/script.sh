#!/bin/bash

set -ex

main() {
    if [[ -n "$TARGET" ]]; then
        cross build --all --target $TARGET

        if [ ! -z $DISABLE_TESTS ]; then
            return
        fi

        cross test --all --target $TARGET
    else
        cargo build --all
        cargo test --all
    fi
}

# we don't run the "test phase" when doing deploys
if [ -z $TRAVIS_TAG ]; then
    main
fi
