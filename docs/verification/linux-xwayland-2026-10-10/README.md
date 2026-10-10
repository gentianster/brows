# Ubuntu 26.04 XWayland GUI evidence

The initial port build, before the appearance fixes, has SHA256
`68a7fa1dc8b2cfcce615256233de21a3ef493a3223b784902882fec394446a3f`.
Tests used fresh `/tmp` fixtures, isolated HOME/XDG/Chromium paths, and a mock
Vivaldi executable that records arguments. Only PID-verified windows started by
the harness were inspected. No real browser, profile, default MIME selection,
or autostart setting was used or changed.

`lifecycle/report.json` records 14 passing checks. Initial display and focus,
WM_DELETE_WINDOW dismissal, hidden resident retention, and another URL returning
the same window all passed. The initial rule launched the mock with fixed arguments
and `--profile-directory=Default`, kept the resident hidden, and an unmatched URL
redisplayed the same window with focus. A subsequent standard close request hid
that window and kept its process. The three published client screenshots show clean Japanese picker text.
The settings screenshot was checked locally but omitted because it contains an
absolute temporary fixture path. The capture files contain mock arguments only.

This is a lifecycle-only pass. This automated run did not verify GUI browser/profile selection, selected FIFO
launch order, or the Cancel button. A later human check selected both synthetic
profiles and its captured arguments matched; it did not verify FIFO or Cancel. The standard
close request exercises the same dismiss path as Cancel, but does not prove the
button responds.

`selection-injection/` preserves the initial-port-build attempt and input diagnostics.
XTest returned success, but XQueryPointer reported own-client coordinates
`[-11, 1018]` after motion intended for `[400, 178]`. The row stayed collapsed and
the mock received no launch. The harness now checks actual pointer arrival and
withholds button input if it fails. No further input tuning was performed.

Focus was accepted in the final lifecycle run, but `earlier-focus-failure/`
records an earlier initial-focus failure. These observations do not establish
that GNOME accepts every focus request.

Reports use relative screenshot names. Temporary fixture paths, PIDs, and window
IDs have been removed for publication; observed check results are retained. The reusable harness and human verification
instructions are in [tests/manual/README.md](../../../tests/manual/README.md).
Preparing fixtures with `--prepare-only` generates a `run-brows.sh` wrapper for
manual profile clicks without launching a real browser.
