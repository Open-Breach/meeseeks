# Meeseeks — Stutter round 3: control and fakesink

The reconnect experiment is finished and failed: on f48f3ed, all six reconnect sessions stuttered at 1 to 6 frames/s with pauses of 1.5 to 1.75 s, worse than any earlier session. This round removes it and answers two questions: does the stutter follow GTK's renderer, as commit 8945792 found, or the machine's state since the earlier smooth sessions; and does Mutter's buffer pool run dry even when the app returns every buffer immediately.

## Where things stand

- `reconnect` is out. Remove the `MEESEEKS_TRY=reconnect` path from the code entirely; do not keep it behind a flag.
- The renderer is an open question, not a settled one. Commit 8945792 found that sessions started soon after an earlier one stuttered at about 19 frames/s with GTK's Vulkan renderer (five of five) and held 60 with GL (two of two), and made GL the default by adding `GDK_DISABLE=vulkan` at startup. Round 1, three hours before round 2 on the same boot, had Vulkan controls smooth. Round 2 ran all six sessions on Vulkan (`GSK_RENDERER=vulkan` was in every command and every session logged `GTK draws with GskVulkanRenderer`). Within round 2 the renderer was constant, but across rounds the evidence contradicts itself. This round runs both.
- The one signal that holds across every round: Mutter logs no `Couldn't dequeue a buffer` before the renegotiation, and starvation begins the moment the real format is negotiated. Its eight-buffer pool is not getting buffers back fast enough. A reconnect hands it a fresh pool to exhaust.
- The pauses are regular, 1.5 to 1.75 s in every round 2 session. Starvation from load is noisy; a period that steady looks like a timer or a timeout. Finding what in Mutter, PipeWire or `pipewiresrc` has that period is part of this round.

## Changes to the app before the run

1. Remove the reconnect code (`RECONNECT_AFTER`, the timer and the `reconnect` field; the buffer-path log on every caps change can stay).
2. Add `MEESEEKS_TRY=fakesink`: build the pipeline as `pipewiresrc ! fakesink sync=false` with no GTK sink and no paintable. The window shows a plain label "fakesink mode" and the Start/Stop button. The frame counter and the 5-second log line keep working, measured at a pad probe on the source pad of `pipewiresrc`, so the log shape is the same as before. Every buffer is returned to PipeWire as soon as the probe has counted it.
3. Keep the buffer-path log line, the `GTK draws with` line and the `held` count exactly as they are.

## The sessions

All on the same boot, same hour, fresh process per session, 60 s of stream each, `RUST_LOG=meeseeks=debug`, Mutter `SCREEN_CAST` topic on for the whole round and off again at the end. Serge rates each session Smooth or Stuttering as before. Start each session within a minute of the previous one ending, since 8945792 tied the Vulkan stutter to that.

| # | Command | Question it answers |
| --- | --- | --- |
| 1, 2 | `RUST_LOG=meeseeks=debug cargo run` (app default: GL, no `MEESEEKS_TRY`) | Is the shipped default smooth on this boot, right now? |
| 3, 4 | `GSK_RENDERER=vulkan RUST_LOG=meeseeks=debug cargo run` | Does Vulkan stutter where GL does not, as 8945792 found, or are both the same, as round 1 suggested? |
| 5, 6 | `MEESEEKS_TRY=fakesink RUST_LOG=meeseeks=debug cargo run` | Does Mutter's pool still run dry with a consumer that returns every buffer at once and draws nothing? |

Run them in this order. If sessions 1 and 2 both stutter, stop after session 4, report, and Serge reboots before anything else is tried.

## What to collect

The same as round 2, so the tables line up:

- Mutter's `Queuing PipeWire buffer` against `Couldn't dequeue a buffer`, split at the renegotiation.
- `Allocating` lines and `Negotiating with device ID` count per session.
- The app's 5-second lines (fps, median, longest, held) and its `GTK draws with` line.
- `pw-top` errors, exit codes, CPU of meeseeks and gnome-shell.

Plus one new item: in every session, the timestamps of each `Couldn't dequeue a buffer` burst, so the gap between bursts can be compared with the 1.5 to 1.75 s pauses.

## How to read the result

| GL (1, 2) | Vulkan (3, 4) | Fakesink (5, 6) | Reading |
| --- | --- | --- | --- |
| Smooth | Stuttering | Smooth, no failed dequeues | 8945792 was right: the fault is in how GTK's Vulkan renderer handles the sink's buffers. Keep GL as the default, close the investigation for the prototype, and file the Vulkan case as a later issue. |
| Smooth | Smooth | Smooth, no failed dequeues | Round 2 was a bad hour, not a bad renderer. Keep GL as the default and watch for it coming back. |
| Stuttering | Stuttering | Smooth, no failed dequeues | The fault is in the GTK sink or GTK's handling of the buffers, whatever the renderer. Next round goes there: `gtk4paintablesink` settings, GTK version, graphics offload. |
| Stuttering | Stuttering | Stuttering, failed dequeues | The fault is upstream of the app: Mutter or PipeWire, or the machine's state. Reboot and rerun 1, 2, 5, 6. If it persists after a reboot, build a minimal reproducer with `gst-launch-1.0` and file a Mutter bug with the logs. |

Any other combination: report it as it is, with the reading left open.

## Separately: the period

While the sessions run, look for the source of the 1.5 to 1.75 s cadence. Candidates to check in order: the `pipewiresrc` `keepalive-time` and `resend-last` properties and their defaults; PipeWire's `core.timeout` and link-level timeouts; Mutter's screen-cast stream code for any timer in that range (`meta-screen-cast-stream-src.c`, look for `g_timeout_add` and `*_ms` constants); the PipeWire graph quantum and rate for the stream node in `pw-top`. Report what matches, if anything, with the file and line.

## Agent prompt

```markdown
Round 3 of the Meeseeks stutter investigation on Open-Breach/meeseeks. Read docs/stutter-round-3.md and your round 2 report first. Do the code changes in "Changes to the app before the run" as one commit on the branch, then hand Serge the six session commands in order; he runs them and rates them. Collect what "What to collect" lists, present the result in the same table shapes as round 2, and give the reading from "How to read the result". Do the period search in parallel and report it in its own section. Do not add any other experiment this round.
```
