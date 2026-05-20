#!/bin/sh
# Fake `claude` binary used by the §15 launch integration test.
#
# Real-claude behaviour we mimic for the PTY pipeline check:
#   1. Emit a non-empty banner to stdout. This is what triggers the
#      drainer's `FirstOutput` event and the bracketed-paste prompt
#      injection per spec §7.
#   2. Read one line of input (the injected prompt) and echo it back so
#      the test can assert the prompt actually reached the child.
#   3. Exit 0.
#
# Args are accepted-and-ignored so we can verify the arg builder feeds
# us the `--model`, `--permission-mode`, etc. tokens without crashing.

printf 'fake-claude ready\n'

# Read the bracketed-paste envelope. The drainer writes
# `ESC[200~ <prompt> ESC[201~ \r`. We tolerate either bracketed or raw
# input — `IFS= read -r line` strips the trailing \r per POSIX shell
# behaviour, so we just dump what we got.
IFS= read -r received
printf 'received-prompt: %s\n' "${received}"

exit 0
