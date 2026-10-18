# Show fixtures

Checked-in `.cmdmx.json` shows for round-trip and migration tests (SHW-03/04).

- `church-demo.cmdmx.json`: the church rig of `cmdmx-core/tests/showfile.rs` (`rig()`) written
  out by `Engine::to_show`: schema 1, profiles embedded, groups 1 and 4, two playbacks, RGB
  colour model. The test `shw_03_fixture_show_is_the_church_rig` compares it byte for byte; when
  the schema changes, regenerate it and add a migration test (SHW-04).
