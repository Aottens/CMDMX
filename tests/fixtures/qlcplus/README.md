# QLC+ fixture definitions (test data, LIB-09)

The `.qxf` files are copied unchanged from the QLC+ project
(https://github.com/mcallegari/qlcplus, `resources/fixtures/`, commit
`3d832cec0042ad937d0da0346e7b953f5952cec7`), licensed under the Apache License 2.0
(https://www.apache.org/licenses/LICENSE-2.0). Authors are named in each file's `<Creator>`.

| File | QLC+ path |
| --- | --- |
| `Shehds-LED-Beam+Wash-19x15W-RGBW-Zoom.qxf` | `Shehds/` |
| `Showtec-LED-Par-56.qxf` | `Showtec/` |
| `Pro-Lights-LumiPar-7QPro.qxf` | `Pro-Lights/` |

Each `<name>.expected.json` is the profile the importer must produce (own model, ADR-006),
written by hand from the spec (`specs/modules/fixture-library.md` LIB-05..10, 15..18).
