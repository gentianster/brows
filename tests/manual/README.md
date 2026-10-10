# Linux GUI verification

Run this manual harness in the target Ubuntu Wayland session after building brows.
It uses the installed Python Pillow package, `libX11`, `libXtst`,
`desktop-file-validate`, and `update-desktop-database`; it installs nothing.

```sh
python3 tests/manual/linux_gui.py \
  --binary target/debug/brows \
  --output /tmp/brows-gui-results
```

The harness creates a fresh short `/tmp/brows-gui-*` fixture and passes isolated
HOME, XDG config/data/runtime/cache paths and Chromium environment variables only
to its subprocesses. The only browser entry is a synthetic native Vivaldi whose
executable records arguments instead of opening a browser. Profile metadata contains
only the Japanese display names 個人 and 仕事用. The current X11 session authentication
is inherited without reading or logging it.

It checks first display and focus, selection followed by hiding, process/window
retention, redisplay and focus on the same X11 window, seven URLs in FIFO order,
fixed and profile arguments, the Japanese settings window, and a separate initial
automatic rule followed by an unmatched URL and Cancel. Captures contain
only the owned client windows. The report records focus failures separately; an
unaccepted focus request causes a failing exit status. Japanese glyphs require
visual inspection of the generated screenshots.

XTest clicks target only a window whose PID matches a process started by the
harness. Before sending input, the harness checks that the target point resolves
to that owned window. It stops if another window covers the target. It does not
read other applications' titles or pixels. Its cleanup quits or terminates only
the processes it started. It never invokes registration, changes default browser
selection, or writes autostart files.

If XTest reports success without moving the pointer to the owned target, button
input is withheld. Use `--lifecycle-only` to check hide/redisplay and the initial
automatic rule through owned-window `WM_DELETE_WINDOW` requests. This mode leaves
GUI selection, selected FIFO launch order, and the Cancel button unverified;
it cannot be treated as a full GUI pass. Focus may be refused by the compositor,
and results from separate runs should be retained.

`report.json`, `application.log`, and six PNG screenshots remain in the output
directory. The report names the fixture containing `capture.jsonl` and isolated
`config.toml`. URLs use `example.invalid`, `auto.invalid`, and `unmatched.invalid`.
Lifecycle-only runs produce four screenshots. `--prepare-only` validates fixtures
without opening an app and creates `run-brows.sh`. The wrapper exports only fixture
variables and inherits current DISPLAY/XAUTHORITY; it never prints session credentials.
The default button coordinates match the current picker;
`--row-y`, `--profile-y`, `--profile-step`, and `--cancel-y` can be adjusted after inspecting an
owned-window screenshot if a theme or font changes its layout.

For a human selection check, prepare a fresh fixture and use its wrapper in the
same graphical session:

```sh
python3 tests/manual/linux_gui.py --binary target/release/brows \
  --output /tmp/brows-gui-human --prepare-only
/tmp/brows-gui-human/run-brows.sh https://example.invalid/manual/first &
# Click 検証用 Vivaldi, then 個人. The picker should hide.
/tmp/brows-gui-human/run-brows.sh https://example.invalid/manual/second
# The same picker should return. Click 検証用 Vivaldi, then 仕事用.
/tmp/brows-gui-human/run-brows.sh https://example.invalid/manual/fifo-1
/tmp/brows-gui-human/run-brows.sh https://example.invalid/manual/fifo-2
# Select each displayed request; the first URL should remain until selection.
/tmp/brows-gui-human/run-brows.sh --quit
```

Inspect only the fixture `capture.jsonl` named by the prepare report to compare
URLs and `--profile-directory` values. The fake launcher opens no browser.
