#!/bin/bash

set -ex

main() {
    if [ -n "$TARGET" ]; then
        cross build --all --target $TARGET

        if [ ! -z $DISABLE_TESTS ]; then
            return
        fi

        if [ ! -z $NO_EXEC_TESTS ]; then
            cross test --all --target $TARGET -- '::tests::'
        else
            cross test --all --target $TARGET
        fi
    else
        cargo build --all
        cargo test --all
    fi
}

# we don't run the "test phase" when doing deploys
if [ -z $TRAVIS_TAG ]; then
    main
fi
