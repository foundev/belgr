# Belgr npm packaging

This directory builds the public `belgr` npm package and its
platform payload packages from an already-published Belgr GitHub release.

Do not publish from a development checkout. `publish-npm.yml` verifies the
release checksums, packages every platform payload, smoke-tests Linux, and
publishes payloads before the root wrapper.
