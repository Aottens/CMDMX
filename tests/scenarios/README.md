# Scenario tests

One JSON file per engine criterion (`"spec": "ENG-nn"`), replayed by
`subprojects/engine/cmdmx-core/tests/scenarios.rs` with a virtual 40 Hz clock.
Format: see `specs/modules/engine-merge.md` → "Scenario test format".

Rules:
- File name starts with the criterion id: `ENG-07-release-mask.json`.
- Values in profiles/cues are 0.0–1.0; expected channels are raw DMX 0–255.
- Keep each file to one behaviour. If you need two, you need two criteria.
