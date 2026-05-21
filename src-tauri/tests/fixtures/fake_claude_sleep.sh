#!/bin/sh
# Fake `claude` binary for SCA-927 stop-escalation real-PTY tests.
#
# Emits a banner (so the drainer fires FirstOutput) and then sleeps
# long enough that the test can exercise the stop-signal ladder
# without racing against a natural exit. The trap handlers let us
# observe which signal actually killed the process — without them,
# `sh` would inherit the default disposition and the test couldn't
# distinguish SIGINT vs SIGTERM in the exit summary.

# Print and flush the banner so the PTY drainer's FirstOutput event
# fires immediately. Without `printf` + explicit newline, line-buffered
# output could stay in the kernel until the read loop drains.
printf 'fake-claude-sleep ready\n'

# Drain the bracketed-paste envelope eagerly so we don't block the
# injector's write. Read with a tiny timeout, then ignore.
IFS= read -r _ignored 2>/dev/null || true

# Sleep for 30s — long enough for the test's stop-escalation ladder
# (up to 13s graceful + slack) without dragging CI if the test fails
# to send any signal. The `wait` lets the shell respond to signals
# immediately rather than after the sleep call returns.
sleep 30 &
wait $!
