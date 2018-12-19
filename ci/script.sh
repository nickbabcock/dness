#!/bin/bash

set -ex

main() {
    cross build --all --target $TARGET

    if [ ! -z $DISABLE_TESTS ]; then
        return
    fi

    cross test --all --target $TARGET
}

# we don't run the "test phase" when doing deploys
if [ -z $TRAVIS_TAG ]; then
    main
fi
