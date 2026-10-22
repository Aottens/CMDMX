# Module spec: engine-merge (ENG)

Owner: `cmdmx-core`. Status: **draft for owner approval**. Behavioural reference: Avolites Titan v19 manual, *Playback Options* and *Cue Playback*.

## Purpose
Turn the set of active sources (programmer, playbacks, masters) into one DMX frame per universe per tick, deterministically, using Titan's HTP/LTP and priority rules.

## Definitions
- **Attribute kind:** `Htp` (intensity/dimmer) or `Ltp` (everything else, including shutter/strobe).
- **Source:** something that proposes attribute values: the programmer, a playback, a master. Each source has a `priority` ∈ {Low, Normal, High, Programmer, VeryHigh} (ordered) and, for playbacks, a `level` 0.0–1.0 from its fader.
- **Tick:** `engine.tick(now: Timestamp)`; `now` is supplied by the caller, never read from the system clock inside core.
- **Frame:** 512 bytes per universe, DMX start code 0.
- **Base value:** the value of an attribute that ENG-01..10 resolve from the active sources
  (priority, HTP/LTP, fades, release, the held `Ltp` value, 0 for an `Htp` attribute nobody
  proposes). Until effects exist, the base value is the merged value.
- **Effect source:** a source (programmer, playback, cue, palette reference) that holds an
  **effect reference** for a fixture and attribute next to, or instead of, a value (ADR-007). An
  effect is **relative** (it adds an offset to the base value) or **absolute** (it replaces the
  base value). What an effect is and how it moves is specified in `effects.md` (phase 2); built-in
  fixture macros on an FX-group channel are ordinary values, not effects.

## Acceptance criteria

### Merge rules
- **ENG-01** For an `Htp` attribute, the output is the highest proposed value among active sources at the highest priority that proposes it. Two Normal-priority playbacks at 60 % and 40 % on the same dimmer → 60 %.
- **ENG-02** For an `Ltp` attribute, the output is the value of the most recently activated source at the highest priority that proposes it. Playback A (pan 10 %) fired at t=1, playback B (pan 90 %) fired at t=2 → 90 %; releasing B returns pan to A's 10 % (A is still active).
- **ENG-03** A source of lower priority never changes an attribute currently owned by a higher-priority source. High-priority spotlight playback owning dimmer 5; firing a Normal playback with dimmer 5 at full → no change on dimmer 5.
- **ENG-04** Programmer values override all playbacks except `VeryHigh`.
- **ENG-05** A `Htp` attribute scales with the playback fader level in fader mode 0 (level 0.5 on a cue with dimmer 100 % → 50 %), while `Ltp` attributes snap to the cue value once the fader passes the trigger point (default 2 %) and do not scale.
- **ENG-06** Grand Master scales all `Htp` output after merge: GM 50 % with merged dimmer 80 % → 40 %.

### Release
- **ENG-07** An `Htp` attribute that no active source proposes outputs 0 (programmer cleared, last playback killed): only `Ltp` attributes hold their last value. When a playback is killed, `Htp` attributes it owned fall to the next source's value or to 0; `Ltp` attributes it owned keep their last value unless the playback's release mask includes that attribute's group.
- **ENG-08** With the release mask including Intensity (Titan strobe-cue pattern), killing the playback returns shutter/strobe to the previous owner's value or the locate value.
- **ENG-09** Release fades over the playback's `release_time` when set, otherwise over the global release time.
- **ENG-10** `Clear` with a typed time fades `Htp` values in the programmer to 0 over that time; `Ltp` values follow the user setting `freeze_current_values` (keep) or `release_to_playback_values` (return to playback).

### Effect layer (ADR-007; no effect sources exist in phase 1, tests land with the first effect task)
- **ENG-14** Per fixture, attribute and tick the output is computed in this order: base value →
  effect → clamp → group masters (GRM-06, `Htp` only) → Grand Master and DBO (ENG-06, PBK-30,
  `Htp` only) → frame writer (virtual dimmer, coarse/fine). With no effect source the output is
  exactly today's.
- **ENG-15** At most one effect acts on an attribute per tick: among the active sources that hold
  an effect for it, the one with the highest priority and, at equal priority, the most recently
  activated (LTP, for `Htp` attributes too: effects do not add up). This choice is made independently of which
  source owns the base value: a programmer circle on Pan/Tilt runs around the position the cue
  list gives.
- **ENG-16** A relative effect adds its offset to the base value; an absolute effect replaces it.
  Either way the result is clamped to 0.0–1.0 (each component for a colour) before the group
  masters, so a Limit master stays a hard limit and the Grand Master scales a dimmer effect like
  any other intensity.
- **ENG-17** An effect is evaluated from the tick's timestamp and its own start time only (ENG-11),
  never from the system clock; two engines fed the same scenario produce the same effect output.
- **ENG-18** The held `Ltp` value (ENG-07) is the base value, never base + offset: when the effect
  stops (its source released or killed, the reference removed), the attribute returns to its base
  value in the next tick, or fades to it as `effects.md` will define.
- **ENG-19** The Take Over pass of GRM-10 compares the master level with the mastered output,
  effect included (what is on stage).

### Determinism and safety
- **ENG-11** Two engines fed the same scenario (same patch, same timestamped actions) produce byte-identical frames at every sampled timestamp.
- **ENG-12** No input combination makes `tick` return an error that stops output: invalid actions are rejected with a `Warning` and the previous frame is retained.
- **ENG-13** `tick` performs no heap allocation in steady state after vertical slice 1 (verified with an allocation counter test; marked `#[ignore]` until the slice is stable).

## Scenario test format
`tests/scenarios/<ID>-<slug>.json`:

```json
{
  "spec": "ENG-01",
  "profiles": { "dimmer": { "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ] } },
  "patch": [ { "id": 1, "profile": "dimmer", "universe": 1, "address": 1 } ],
  "playbacks": [
    { "id": 1, "priority": "Normal", "fader_mode": 0, "values": { "1": { "Dimmer": 1.0 } } },
    { "id": 2, "priority": "Normal", "fader_mode": 0, "values": { "1": { "Dimmer": 1.0 } } }
  ],
  "actions": [
    { "t": 0.0, "fader": { "playback": 1, "level": 0.6 } },
    { "t": 0.0, "fader": { "playback": 2, "level": 0.4 } }
  ],
  "expect": [ { "t": 0.1, "universe": 1, "channels": { "1": 153 } } ]
}
```

Profiles use the model of `fixture-library.md` (full shape with `modes`, or the minimal `{ "attributes": [...] }` above = one mode "default"); a patch entry may name a `mode` (default: the first). Values are 0.0–1.0 in profiles/cues; expected channels are raw DMX 0–255 (`round(v * 255)`). The runner in `cmdmx-core/tests/scenarios.rs` loads every file, replays actions in time order with a virtual clock, ticks at 40 Hz, and asserts each expectation.

## Out of scope here
Fade curves, fixture overlap, effect shapes and parameters (own specs; `effects.md` in phase 2). 16-bit attributes are covered in `patch.md` (coarse/fine channel pairs) and must be respected by the frame writer (ENG tests use 8-bit only).
