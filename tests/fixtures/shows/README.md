# Show fixtures

Checked-in `.cmdmx.json` shows for round-trip and migration tests (SHW-03/04).

- `church-demo.cmdmx.json`: the church rig of `cmdmx-core/tests/showfile.rs` (`rig()`) written
  out by `Engine::to_show`: schema 9, profiles embedded, groups 1 and 4, two playbacks, RGB
  colour model, the output nodes of universes 1 and 2. The test `shw_03_fixture_show_is_the_church_rig` compares it byte for byte; when
  the schema changes, regenerate it and add a migration test (SHW-04).
- `church-demo-v1.cmdmx.json`: the same rig as written by schema 1, kept untouched for the
  1 -> 2 migration test (`pal_23_a_version_1_show_is_migrated`).
- `church-demo-v2.cmdmx.json`: the same rig as written by schema 2, kept untouched for the
  2 -> 3 migration tests (`pbk_45_*` in `cmdmx-core/tests/playbacks.rs`).
- `church-demo-v3.cmdmx.json`: the same rig as written by schema 3, kept untouched for the
  3 -> 4 migration test (`cue_38_*` in `cmdmx-core/tests/cuelists.rs`).
- `church-demo-v4.cmdmx.json`: the same rig as written by schema 4, kept untouched for the
  4 -> 5 migration test (`grm_13_*` in `cmdmx-core/tests/showfile.rs`).
- `church-demo-v5.cmdmx.json`: the same rig as written by schema 5, kept untouched for the
  5 -> 6 migration test (`out_27_*` in `cmdmx-core/tests/output.rs`).
- `church-demo-v6.cmdmx.json`: the same rig as written by schema 6 (groups hold fixture ids),
  kept untouched for the 6 -> 7 migration test (`shw_04_*` in `cmdmx-core/tests/cell_store.rs`).
- `church-demo-v8.cmdmx.json`: the same rig as written by schema 8, kept untouched for the
  8 -> 9 migration test (`prg_83_*` in `cmdmx-core/tests/selection_record.rs`).
