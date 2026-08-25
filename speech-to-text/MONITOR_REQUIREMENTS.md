# Interactive Streaming Monitor Requirements

## Purpose

`dg-stt stream file` and `dg-stt stream microphone` can display an interactive
dashboard for one or more parallel Deepgram streaming connections. The monitor
is intended for observing connection health and recent transcription activity
while keeping the existing non-interactive text and JSON modes unchanged.

## Invocation

```powershell
cargo run --manifest-path speech-to-text/Cargo.toml -- `
  stream file --file $env:AUDIO_FILE --connections 3 --monitor

cargo run --manifest-path speech-to-text/Cargo.toml -- `
  stream microphone --connections 2 --monitor
```

`--monitor` and `--output json` are mutually exclusive. Connections are created
from the `--connections` value at startup; adding new connections interactively
is not supported because the audio fan-out is configured before streaming starts.

## Functional requirements

- Show one row for each configured connection.
- Show a request ID suffix after a connection is established, or a pending value
  before then.
- Show connection states using consistent colors: connecting, connected,
  streaming, closed, and error.
- Show the most recent ten transcript words for each connection.
- Show the time taken to establish each connection.
- Allow Up/Down selection of a connection row.
- Allow `d` to terminate the selected connection.
- Allow `q`, Escape, or Ctrl+C to stop all connections and exit.
- Stop cleanly when file streaming completes or all connections close.
- Restore the terminal's original screen, cursor visibility, colors, and raw-mode
  state on normal exit, Ctrl+C, monitor errors, and task failures.
- Continue to support callbacks and all existing Deepgram request options.

## Non-functional requirements

- Do not block audio forwarding or Deepgram response handling on rendering.
- Keep the monitor responsive while connections are active.
- Work in terminals that support crossterm alternate-screen and raw-mode APIs.
- Truncate long request IDs, statuses, and transcript text to the current terminal
  width without panicking.
- Handle terminal widths and heights smaller than the dashboard layout.
- Keep diagnostic errors available through stderr and `dg-stt-debug.log`.

## Manual test plan

The following tests require a valid `DEEPGRAM_API_KEY` and representative audio.

1. Start file monitoring with one connection. Confirm the dashboard opens, the
   row moves from connecting to connected/streaming, words appear, and the
   terminal returns to its original screen after completion.
2. Start file monitoring with `--connections 3`. Confirm three rows appear,
   each receives its own request ID, and all rows update independently.
3. Press Up/Down and `d`. Confirm only the selected connection closes while the
   remaining connections continue.
4. Press `q`, Escape, and Ctrl+C in separate runs. Confirm all connections stop
   and the shell prompt has normal cursor visibility, colors, and input mode.
5. Run microphone monitoring and speak several short phrases. Confirm recent
   words update without leaving transcript text over the dashboard.
6. Use a deliberately invalid endpoint. Confirm the error state is visible and
   the terminal is restored after the process exits.
7. Run with `--monitor --output json` and confirm clap rejects the conflicting
   options before opening the terminal.
8. Run normal streaming without `--monitor` and confirm the existing formatted
   transcript output is unchanged.
9. Resize the terminal to a narrow and short window. Confirm the monitor remains
   usable and does not panic.

## Known scope boundaries

- The monitor observes the connections created at startup; it does not create
  new connections dynamically.
- Raw JSON output is available through `--output json`, not simultaneously with
  the monitor dashboard.
- Mouse interaction is not currently implemented; every monitor action has a
  keyboard equivalent.
