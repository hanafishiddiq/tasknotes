# Ticket 16 — Quiet ambient and interaction sound: plan (2026-10-26)

Started on the owner's phase-transition instruction (Tickets 05–18 autonomy: Draft PR per ticket, AI review gate, no merge, synthetic data only). Built on `integration/tasknotes-v1-07-18` at 57d0ec6 (Tickets 07–10 and 14). Frontend only: no schema, core, Tauri command or `src-tauri` change, so it stays clear of Ticket 11 (core + CLI) and Ticket 15 (menu bar, close-to-quit, global Quick Capture) running in parallel.

Scope: spec stories 45–47, the "Separate sound controls" and "Third-party ambient/audio assets" lines, and the Reduce Motion / accessibility criterion of the ticket.

## Shape

- `src/lib/sound/rain.ts` — the soft rain, synthesized in code (see Provenance). Pure function, seeded, deterministic.
- `src/lib/sound/sound-engine.ts` — the one deep module: `apply(settings)`, `gesture()`, `play(cue)`, `fadeOut()`, `status()`, `dispose()`. It owns every Web Audio rule (fade-in from silence, autoplay wait, mute ramp, suspend when silent, lifecycle). It takes a `createContext` factory, so tests drive it with a fake AudioContext.
- `src/lib/sound/sound-settings.ts` — defaults (interaction 12%, ambient 8%, both on) and persistence under `localStorage["tasknotes.sound"]`, the same per-window pattern Ticket 14 used for the theme (`tasknotes.theme`). Unreadable or missing values fall back per channel to the defaults. Why localStorage and not Ticket 15's Rust `shell-settings.json`: sound is only played and read by the page, so the page keeps it like the theme; shell settings exist because Rust must read them (the global shortcut, menu-bar mode) when no page is loaded.
- `src/lib/sound/use-sound.ts` — React glue: one engine per window, gesture/visibility listeners, a quit fade via the existing `saveBeforeQuit` seam.
- `src/components/SoundSettings.tsx` — its own Settings section. `SettingsView` only gained a `children` slot rendered after Appearance, so Ticket 15 can add its section the same way without touching this one.
- `App.tsx` — three lines: `useSound()`, a cue after a Task is created, and a cue when the core's answer turns a Task Completed (status or 100% progress — the core decides, the UI does not repeat the rule).

## Decisions made by the implementer (owner may revisit)

- **saya pilih synthesis for both channels** karena it needs no bundled recording, so nothing has to pass a license review and nothing is downloaded; it works offline by construction.
- **saya pilih a pre-rendered 12 s stereo loop played by one `AudioBufferSourceNode`** karena all filtering is baked in once, so the audio thread only replays a buffer (no per-frame JS, no ScriptProcessor, no live filter nodes). The loop is rendered in slices of about 1 ms (16,384 samples) with a pause between them, starting as soon as the rain is wanted and the audio context exists, which is before any click; the first click only starts the finished buffer. Cost: the same one-time 60–80 ms of work, spread over about 12 short slices (reviewed 2026-10-26: it used to run in one stretch on the first click). If the render is not finished when the context is allowed to run, the rain starts the moment it finishes. Muted ambient never renders it.
- **Rain always fades in from silence over 2 s; mute and volume changes ramp over 80 ms on both channels (a cue that is playing when interaction is muted fades out instead of clicking); the quit fade is 300 ms.** No ramp is ever scheduled while autoplay is blocked, so the first audible moment after unblocking still starts at 0.
- **Autoplay blocked → status "waiting-for-gesture"**: the engine keeps asking `resume()` on the next pointer-down or key-down (capture phase, so it works inside WebKit's gesture window). Settings then says "Sound starts after your first click or key press." Interaction cues requested while blocked are dropped, never queued. After the engine suspends the context itself (both channels muted), un-muting resumes it without showing that hint; the hint only shows if the resume does not actually start audio.
- **Both channels muted (or at 0%) → the AudioContext is suspended** after the fade, so muted audio costs no audio-thread CPU.
- **Web Audio missing or failing → status "unavailable"**; Settings says so and every control keeps working; Task actions are unaffected. Sound never gates any behavior, and Reduce Motion does not change sound (sound is not motion).
- **Lifecycle (spec story 46):** minimized (page hidden) keeps the rain; quitting fades it out before the app answers the quit request. If a quit is held back (a Note could not save), the rain returns on the next click or key press.
- **Cues:** create = one soft sine note (E5, 160 ms); complete = two soft sine notes rising a fifth (C5 → G5, ~340 ms total). Envelope peaks at 0.7 before the channel gain, so at 12% the peak is about −21 dBFS (0.7 × 0.12).
- **Volume mapping is linear** (slider % = gain). Moving a slider does not un-mute the channel.

## Integration point for Ticket 15 (menu-bar mode)

There is no presence API. Only quitting changes sound, so the engine keeps one internal `quitting` flag: `fadeOut()` sets it, and the next `gesture()` clears it (a held-back quit). The hook no longer listens to page visibility: a minimized window, or one hidden to the menu bar, keeps the rain with no code at all. When Ticket 15 merges:

- Hidden to the menu bar: nothing to call; the rain continues.
- Close-to-quit: it already arrives as the quit request, so the 300 ms fade runs through `saveBeforeQuit`. If closing only hides the window, the rain continues.

If the owner wants a different policy for the menu bar, add the smallest input then; nothing needs it today.

## Provenance of the ambient sound (owner decision 2026-10-25)

Generated in code by `src/lib/sound/rain.ts`; no third-party recording, sample or library. Per channel (two seeds, so left and right differ): pink noise (Paul Kellet's filter) → one-pole low-pass at 1.6 kHz → one-pole high-pass at 140 Hz, with a ±12% swell repeating twice per loop; plus about 14 drops per second, each a sine tick at 700–2,200 Hz with a 4–14 ms exponential decay and random level; tail cross-faded (equal power, 250 ms) into the head; scaled to RMS 0.16 with peak ≤ 0.85. Seeded PRNG (mulberry32), so the same seed always gives the same rain. Interaction cues are sine oscillators with a gain envelope, also generated at play time. Nothing in the repository is a recorded or licensed audio file.

## Vertical TDD slices

1. Rain generator: deterministic per seed, bounded and not clipped, seamless loop (wrap jump below the 99th-percentile step across 12 channels), low treble.
2. Engine: default 8% rain rising from silence; blocked autoplay waits silently, then fades in from 0 on a gesture.
3. Engine: mute within 80 ms and stop the loop; volume change without restart; 12% cue; muted cue silent; cue dropped while blocked; both muted → context suspended, then back.
4. Engine lifecycle: minimized and menu-bar keep rain; quit fade then recovery on gesture; recovery after a system interruption; unavailable Web Audio; dispose and restart.
5. App: Sound section defaults; independent mute/volume persisted across a remount (restart); cues on create and complete and silence when muted; no Web Audio + Reduce Motion keeps Settings and Task completion working; blocked autoplay starts on first click; quit fades the rain before `quit_ready`.

## CPU measurement (implementer, headless browsers via Playwright, 20 s windows)

Real `sound-engine.ts` + `rain.ts` bundled with Vite, 48 kHz, process-tree CPU from `ps`:

| Scenario | Chromium headless shell 1248 |
|---|---|
| Idle page, no audio | 0.05% of one core |
| Defaults (rain 8% + interaction 12%) | 1.0% |
| Defaults + a "complete" cue every second | 0.9% |
| Both channels muted (context suspended) | 0.1% |

One-time rain render: 62.5 ms (Chromium), 82 ms (WebKit). WebKit's audio runs in XPC processes outside the measured process tree, so its CPU figures (≈0%) are not evidence; it did show the context reaching `running` after the first click. The real Tauri WKWebView still needs the owner's native check below.

## For owner review (sound character and visuals not settled by DESIGN.md or the spec)

1. Rain character: soft, dark (1.6 kHz low-pass), sparse drops; loop length 12 s.
2. Cue sounds: E5 tick on create, C5→G5 on complete; no cue for other actions (e.g. status changes other than Completed, Remove, Undo).
3. 2 s fade-in when the rain starts and after un-muting.
4. Linear volume curve; slider does not un-mute.
5. Section label "Ambient rain"; mute as a speaker icon button left of each slider, with the % shown on the right.
6. Rain keeps playing while the window is minimized or hidden to the menu bar (spec 46); stops only on quit.

## Owner listening / CPU checklist (native macOS, synthetic data)

1. Fresh launch: within a couple of seconds rain is faintly audible, rising gently (no burst). If silent, click once anywhere: it fades in; Settings → Sound no longer shows "Sound starts after your first click…".
2. Listen for 1 minute with headphones: no click or gap every 12 s; nothing harsh or hissy.
3. Add a Task → one soft tick. Mark a Task Completed (status or 100% progress) → soft two-note chime. Neither is louder than a quiet system sound at 12%.
4. Settings → Sound: mute ambient → silent within a blink; drag Ambient to 30% and back; mute interaction → completing a Task is silent; the other channel never changes.
5. Quit and reopen: both sliders and mutes are as you left them.
6. Minimize the window: rain continues. Cmd+Q: rain fades out briefly, app quits, no sound remains.
7. Activity Monitor → Tasknotes (and its WebContent / GPU processes) CPU: rain on vs both muted, 1 minute each. Expect low single-digit % with rain, near idle when muted.
8. Turn on Reduce Motion and mute both channels: creating, completing, searching and Settings still work.
