# Main-screen UX research for iteration 2

Date: 2026-10-17
Scope: input for iteration 2 of the main-screen sketch (UX-01). It covers what operators say about current console UIs, UX principles specific to show control, a list of components the main screen needs, a visual direction that avoids the "AI dashboard" look, and notes specific to the Mac. Target user: an experienced Avolites programmer running Sunday services (about 30 fixtures) and sometimes bigger shows, on a Mac with a trackpad or mouse, sometimes a touchscreen, plus a BCF2000.

Method note: Reddit (r/lightingdesign, r/techtheatre) could not be fetched or indexed from this environment, and several forums returned 403. Operator opinions therefore come from ControlBooth, the ETC community, the QLC+ forum, trade press and vendor release notes. Claims marked *(convention)* are common practice I could not tie to a source. Treat them as hypotheses to check with the owner.

---

## 1. What operators love and hate in console UIs

### Avolites Titan (the behavioural reference)
- **Loved: visual, fast to program, quick legends.** A grandMA3-vs-Titan comparison calls Titan the more visual and intuitive console, faster for quick programming, with hand-drawn "Quick Sketch" legends crews can read at a glance ([fpcevents](https://fpcevents.com/blog/grandma3-vs-avolites)). Long-time users say early Titan was poor but became very capable by v7–v9 ([ControlBooth](https://www.controlbooth.com/threads/considering-a-new-avolites-console.39529/latest)).
- **Palette buttons carry a lot of information.** The palette number sits top-left, the G/N/S type top-right, and the IPCGBES groups the palette contains sit below the legend ([Avolites manual – creating palettes](https://manual.avolites.com/docs/palettes/creating-palettes)). Colour palettes get an auto-generated colour legend, and gobo palettes show the gobo image ([workspace windows](https://manual.avolites.com/docs/titan-basics/workspace-windows)).
- **Feedback states that are worth copying:**
  - A palette button lights when it is in use in the programmer ([using palettes](https://manual.avolites.com/docs/palettes/using-palettes)).
  - Palettes that do not apply to the selected fixtures grey out ("Filter Relevant Palettes", same page).
  - Selected fixture buttons turn blue ([controlling fixtures](https://manual.avolites.com/docs/quick-start/controlling-fixtures)).
  - Handles turn yellow during a release fade, because they still affect output ([v17 release notes](https://www.avolites.com/Portals/0/Downloads/ReleaseNotes/TitanSuite/TitanReleaseNotesV17.0.pdf)).
  - Next to the wheels, the IPCGBESFX indicators mark the active group with a grey box and modified groups with a blue circle ([other parts of the touch screen](https://manual.avolites.com/docs/titan-basics/other-parts-of-the-touch-screen)).
  - The menu bar turns red when Menu Latch is on (same page).
- **Undo is visible.** The prompt area lists recent actions, and touching it opens an Undo History in which undone steps are greyed out and can be redone. Actions that cannot be undone carry a "not allowed" symbol (same page).
- **Halos.** Fixture, group, palette and playback buttons can carry a custom coloured halo, and on D7/D9 the playback halo colour also lights the fader LEDs ([workspace windows](https://manual.avolites.com/docs/titan-basics/workspace-windows)). This maps directly onto the BCF2000 bank on screen.
- **Quick Palettes.** Pressing a palette with nothing selected applies it to every fixture it is relevant to ([using palettes](https://manual.avolites.com/docs/palettes/using-palettes)). This is a key busking behaviour, and the UI must make it obvious when nothing is selected.
- **Pain points:**
  - Free-floating windows need manual sizing (corner drags, a resize grid, "Resize Neighbours"), so layout becomes a chore ([workspace windows](https://manual.avolites.com/docs/titan-basics/workspace-windows)).
  - v17 had to fix slow window opening "following feedback from users" ([v17 notes](https://www.avolites.com/Portals/0/Downloads/ReleaseNotes/TitanSuite/TitanReleaseNotesV17.0.pdf)).
  - The PC Suite runs on Windows 10 only (same notes). Avolites does not support running it under Parallels on Apple-silicon Macs, and users report freezes and no DMX output ([Avolites forum](https://forum.avolites.com/viewtopic.php?p=25044), [Parallels forum](https://forum.parallels.com/threads/avolites-t2.359715/)). This is the gap ColourmotionDMX fills: a Titan workflow that runs natively on a Mac.
- **Recent Titan UI direction:**
  - v14 added on-screen Virtual Faders (15 pageable handles) and a streamlined Handle Options ([LSi](https://lsionline.com/news/avolites-releases-v14-titan-lighting-software)).
  - v17 improved the colour picker so it applies to all colour models in a selection regardless of selection order ([v17 notes](https://www.avolites.com/Portals/0/Downloads/ReleaseNotes/TitanSuite/TitanReleaseNotesV17.0.pdf)).
  - v18 lets you press a group button and then pick a softkey on the same multitouch screen ([audioeffetti](https://audioeffetti.com/it/blog/avolites-titan-18-0-upgrades-new-features-and-enhancements-n170851)).

### grandMA3 (and MA2)
- **Loved:** flexibility and the encoder/executor combination. One programmer calls MA3 "quite revolutionary" because he can shape the workflow around the show ([ALIA](https://alia.com.au/rohan-thornton-chooses-an-ma-lighting-grandma3-onpc-command-wing-xt)).
- **Hated:** MA2 was criticised as keystroke-heavy with too many menus ([ETC community, Hog vs MA2](https://community.etcconnect.com/control_consoles/legacy-consoles/f/legacy---wholehog-3/49656/hog-over-grandma2/159036)). MA3 has a steeper learning curve and a phaser workflow that needs maths ([fpcevents](https://fpcevents.com/blog/grandma3-vs-avolites)).
- **Fixed screen anatomy:** a control bar, a user-defined area for views, a command line, an encoder bar with the Grand Master on its right, and a view bar of stored views ([MA3 first view](https://help.malighting.com/grandMA3/2.5/HTML/qsg_first_view.html)). Bars can be hidden and the onPC UI scaled for high-DPI screens ([display configuration](https://help.malighting.com/grandMA3/2.5/HTML/ws_ui_display_configuration.html)). **Lesson:** fixed bars around one flexible area.
- **Colour semantics are the most complete in the industry** ([MA3 system colours](https://help.malighting.com/grandMA3/2.5/HTML/ws_colors_system.html), [markers](https://help.malighting.com/grandMA3/2.5/HTML/ws_colors_markers.html)):
  - White on red means an active programmer value. Red text on grey means a value is in the programmer but inactive and will not be stored.
  - Cyan means a new or higher value in the cue, green a lower value, magenta a tracked value, white a blocked value.
  - Selected fixtures are yellow on grey. A dim orange marks fixtures in the selection but outside the current sub-selection.
  - Bright cyan marks values from a preset, blue marks parked attributes, purple marks phaser values.
- **Feature-group bar:** radio buttons. The active group is yellow, groups the selection does not have are greyed out, and a coloured marker shows a group with active programmer values ([feature group control bar](https://help.malighting.com/grandMA3/2.5/HTML/ws_eb_feature_group_control_bar.html)). This is the same idea as Titan's IPCGBESFX tabs.
- **Themes:** MA ships a "default" theme and a bolder "defaultDAYLIGHT" theme for bright conditions ([colour themes](https://help.malighting.com/grandMA3/2.4/HTML/ws_colors_color_theme.html)). **Lesson:** a dark theme by default, plus a high-contrast variant for daylight.
- **Keyboard shortcuts toggle.** On onPC, the ShCuts button or F10 toggles them, and the button's text colour shows the state. Shortcuts switch off automatically in text pop-ups ([keyboard shortcuts](https://help.malighting.com/grandMA3/2.5/HTML/do_shortcuts_keyboard.html)). This solves the Mac problem of typing in a text field and accidentally firing a cue.

### ETC Eos / Apex / Nomad
- **Colour conventions** ([ETC support](https://support.etcconnect.com/ETC/Consoles/Eos_Family/Software_and_Programming/Channels_Are_Stuck_On_and_Have_a_Colored_Value)): red is a manual value that has not been recorded. Blue is up from the previous cue, green is down, magenta is tracked, white is blocked, yellow is from a submaster. Gold numbers mark captured channels and a gold outline marks the selected channel ([Eos display conventions](https://www.etcconnect.com/uploadedFiles/Main_Site/Content/Support/Articles/Consoles/Eos/Eos-Ion%20Displays%20Conventions.pdf)).
- **Cross-vendor rule: red means manual or programmer.** Titan, MA and Eos all reserve red (or white on red) for "you touched this and haven't stored it yet". Do not use red for anything decorative.
- **Apex hardware choices** ([LSi](https://lsionline.co.uk/news/etc-extends-eos-range-with-apex-console), [PLSN](https://plsn.com/?p=353115)):
  - Target keys have OLED legends, so physical buttons show what they do.
  - A haptic touch strip above the keypad.
  - Brightness-adjustable 4K touchscreens.
  - **Lesson:** labels belong next to the controls they describe.
- **Eos v3.0:** a GUI overhaul with drag-to-rearrange tabs, plus more information on the fader, palette, preset and group displays ([ETC/alia](https://alia.com.au/?p=41850)). v3.3 redesigned the faders with customisable layouts and multitouch ([ETC news](https://www.etcconnect.com/About/News/ETC-Announces-Eos-v3-3-Software-with-Expansion-Processing-and-Much-More.aspx)).
- **Nomad complaint:** one user said it feels like "an old DOS program" rather than a native app ([ETC community](https://community.etcconnect.com/control_consoles/eos-family-consoles/f/eos-family/15066/what-is-the-correct-pc-program/58833)).
- **Magic Sheet lesson:** a user found that colouring channel outlines to match the gel worked better than colour fills ([ETC community](https://community.etcconnect.com/control_consoles/eos-family-consoles/f/eos-family/12208/share-your-magic-sheets/48178)). This applies directly to colour legends in a dark room. Users also warn that on-screen colour can disagree with real output ([ETC community](https://community.etcconnect.com/control_consoles/eos-family-consoles/f/eos-family/29905/magic-sheet-colors-display-wrongly)), so show a computed output swatch and do not trust stored legends.

### High End Hog 4 / Hog 5
- **Hog v5.0 (2025) "User Interface Style Refresh":** removed superfluous lines, gradients and faux-3D buttons for a "flatter, bolder" look ([ALPD](https://www.thealpd.org.uk/news/313846/etc-releases-hog-v501-software), [JP release notes](https://www.ushio-pro.com/Portals/0/images/entertainment/data/hog_release/Hog_v5-0-0-937_releasenotes_JP.pdf)).
- It merged the playback bar, command line, encoder wheels and selection toolbars into one "Design Toolbar" for single-monitor use (same notes). **Lesson:** this is the industry's own answer to a one-screen laptop layout, and it is the closest model for a Mac.
- **Hog 4 v4.0 (2024):** a new tabbed Colour Picker, plus a "Colour Mix" preview column in output and editor windows that shows what colour-mixing fixtures are outputting ([ETC news](https://www.etcconnect.com/About/News/ETC-Announces-Major-Update-to-Hog-4-Operating-System-with-v4-0.aspx)).
- **Complaints from Hog 4 PC users:** the on-screen wheel count did not match the physical wing, and encoders froze or jumped ([ETC community](https://community.etcconnect.com/control_consoles/hog-lighting-controls/f/hog-os-software-discussion/41273/a-few-hog-4-pc-issues-and-questions)). **Lesson:** the on-screen encoder display must match the BCF2000 exactly.

### ChamSys MagicQ, Onyx, Mac apps
- **MagicQ** has three window kinds: box grids (groups, palettes), spreadsheets (patch, cues) and free-form (plot, colour picker, timeline). Changing windows never affects programming or playback ([MagicQ layout](https://docs.chamsys.co.uk/magicq/manual/console_layout.html)). Its PC interface is said to feel out of step with native PC software ([Onstage Lighting](https://www.onstagelighting.co.uk/training-tutorials/magicq-pc-intro/)). Users note MA allows more per-button customisation than ChamSys ([Thomann reviews](https://www.thomannmusic.com/chamsys_magicq_mq70_reviews.htm)).
- **Onyx 4.6** redesigned "hundreds" of items for readability and added a virtual playback overlay so PC users reach playbacks without leaving their workspace ([ALIA](https://alia.com.au/obsidian-releases-onyx-version-4-6)).
- **Lightkey (Mac):**
  - Reviewers praise how easy it is to learn ([learnstagelighting](https://www.learnstagelighting.com/blog/should-i-use-lightkey-full-lightkey-app-review%2F)).
  - Fully Mac-native: full-screen single window, Dark Mode, trackpad gestures with haptics, unlimited undo, autosave, extensive keyboard shortcuts ([lightkeyapp.com](https://lightkeyapp.com/)).
  - "Bubbles" in the Live Preview show which properties are overridden or come from a preset, sequence or cue (same page). This is a visual answer to "where does this value come from".
- **Luminair:** one user called it slow to program and "a bit clunky" ([ControlBooth](https://www.controlbooth.com/threads/luminair-ipad-app.46162/latest)).
- **QLC+ 5:** v4 users were put off by oversized default widgets and the generally clunky UI that came with the move to touch-first ([QLC+ forum](https://www.forum.qlcplus.org/viewtopic.php?p=68321)). **Anti-pattern:** making everything big for touch costs density, and experienced users leave.

### General operator principles
- Consistent layouts let hands work by feel so attention stays on stage. A console needs one clear, reliable way to do each thing, not the several paths desktop apps offer. It should be built for this one task rather than borrowing desktop idioms wholesale ([Onstage Lighting – lighting control UI](https://www.onstagelighting.co.uk/lighting-equipment/lighting-control-ui/)).
- Familiar keystrokes make console choice personal ([ALIA LightShark review](https://alia.com.au/review-lightshark-ls-1)). Copying Titan's exact names, positions and order is a feature, not a lack of imagination.

---

## 2. UX principles for show control

| Topic | Finding | Source |
|---|---|---|
| Dark room | Status colours that pass in the office can fail in a dark, coloured-light room. A navy-bridge UI had to make its "active" green lighter and more saturated after testing on site. Test the palette at the church with stage light spilling onto the screen. | [MouseUX](https://mouseux.com/design-and-characterization-of-interfaces-in-different-lighting-conditions/?lang=en) |
| Dark room | Ship a dark default theme plus a bolder daylight theme. Do not use pure black: it smears on many panels, and dark grey gives room for elevation *(Material dark theme guidance)*. | [MA3 themes](https://help.malighting.com/grandMA3/2.4/HTML/ws_colors_color_theme.html), [Material dark theme](https://m2.material.io/design/color/dark-theme.html) |
| Dark room | Prefer coloured outlines, strips or swatches over large saturated fills, which glare and wash out the eye. | [ETC community](https://community.etcconnect.com/control_consoles/eos-family-consoles/f/eos-family/12208/share-your-magic-sheets/48178) |
| Fixed positions | Fixed bars (command line, encoders, playbacks, view buttons) around one configurable area. Windows never reflow when content changes. Changing windows never changes the show state. | [MA3 first view](https://help.malighting.com/grandMA3/2.5/HTML/qsg_first_view.html), [MagicQ](https://docs.chamsys.co.uk/magicq/manual/console_layout.html) |
| Fixed positions | Grids keep fixed rows and columns, so button 7 is always in the same place. Titan locks the grid with Rows/Columns settings. | [Titan workspace windows](https://manual.avolites.com/docs/titan-basics/workspace-windows) |
| Live / programmer / blind | One cross-vendor rule: red means programmer or manual (not stored). Tracked or inherited values are magenta. Preset or palette-sourced values are cyan. Values above or below the previous cue are blue or cyan and green. | MA3, Eos sources above |
| Live / programmer / blind | Blind must be unmistakable. Eos tells Live from Blind mostly by display focus and tab, which users fight with ([ETC community](https://community.etcconnect.com/control_consoles/eos-family-consoles/f/eos-family/15325/feature-request-bug-report-question-please-stop-automatically-switching-between-live-and-blind-and-changing-display-setup-archive)). Recommendation: a full-width coloured frame or bar while Blind is on *(convention)*. | — |
| Selection | Show selected, in-selection-but-not-current (Next/Prev sub-selection) and selection order as distinct states. MA uses yellow, dim orange and a separate marker. | [MA3 system colours](https://help.malighting.com/grandMA3/2.5/HTML/ws_colors_system.html) |
| Encoders | Group tabs show three states: active, available for the selection, and has programmer values (Titan: grey box and blue circle; MA: yellow, greyed and coloured marker). The on-screen wheel labels must match the physical controller 1:1. | [Titan touch screen](https://manual.avolites.com/docs/titan-basics/other-parts-of-the-touch-screen), [MA3 FG bar](https://help.malighting.com/grandMA3/2.5/HTML/ws_eb_feature_group_control_bar.html), [Hog forum](https://community.etcconnect.com/control_consoles/hog-lighting-controls/f/hog-os-software-discussion/41273/a-few-hog-4-pc-issues-and-questions) |
| Encoders | Titan's roller display shows the value range around the current value, and touching the upper or lower segment sets maximum or minimum. | [Titan touch screen](https://manual.avolites.com/docs/titan-basics/other-parts-of-the-touch-screen) |
| Colour picker | Modern pickers offer a hue/saturation field, linked RGB/CMY/HSB faders, a gel "swatch book" with filter and sort, and for multi-emitter fixtures a quality fader. MA turns the brightness fader red above 100%. | [MA3 colour picker](https://help.malighting.com/grandMA3/2.5/HTML/operate_color_picker.html), [Hog v4](https://www.etcconnect.com/About/News/ETC-Announces-Major-Update-to-Hog-4-Operating-System-with-v4-0.aspx) |
| Colour picker | Apply to all colour systems in the selection regardless of selection order (Titan v17). Show the computed output colour per fixture (Hog's "Colour Mix" column). | [Titan v17](https://www.avolites.com/Portals/0/Downloads/ReleaseNotes/TitanSuite/TitanReleaseNotesV17.0.pdf) |
| Legends | Text, picture or auto-generated legends. Colour palettes auto-show their colour and gobo palettes their gobo image. Optional halo colour per button. | [Titan workspace windows](https://manual.avolites.com/docs/titan-basics/workspace-windows), [creating palettes](https://manual.avolites.com/docs/palettes/creating-palettes) |
| What's playing | An active palette lights up, a running playback is highlighted, and a playback still fading out stays marked (Titan's yellow during release). Show the cue number, next cue and fade progress on the handle itself. | [using palettes](https://manual.avolites.com/docs/palettes/using-palettes), [Titan v17](https://www.avolites.com/Portals/0/Downloads/ReleaseNotes/TitanSuite/TitanReleaseNotesV17.0.pdf) |
| Undo / safety | A visible undo history, with irreversible actions flagged before you commit. A desk-lock / venue mode that leaves only basic playbacks usable (Titan "Venue Mode workspace" on the lock screen). | [Titan touch screen](https://manual.avolites.com/docs/titan-basics/other-parts-of-the-touch-screen), [workspace windows](https://manual.avolites.com/docs/titan-basics/workspace-windows) |
| Command line vs direct | Titan works mostly by direct manipulation (touch buttons plus softkeys) with a prompt that echoes the syntax. Eos and MA are command-line first. For a Titan user, the prompt line should always be visible but optional to type in. | [Titan quick start](https://manual.avolites.com/docs/quick-start), [ETC community](https://community.etcconnect.com/control_consoles/congo-cobalt-family-consoles/f/cobalt-family/17635/is-nomad-recommended-for-tech-capable-beginners-should-we-look-at-others/66492) |
| Customisation | Store and recall workspaces (Titan: workspace buttons, and set lists can recall a workspace per song). Keep it to a few presets rather than free-floating windows. | [Titan Mobile manual](https://manual.avolites.com/docs/about-the-consoles/titan-mobile), [workspace windows](https://manual.avolites.com/docs/titan-basics/workspace-windows) |
| Density vs touch | Touch-first oversizing alienates experienced users (QLC+ 5). Apple's minimum touch target is 44 pt. A mouse-first Mac UI can go denser; aim for a 44–56 pt grid cell, so it works on a touchscreen without looking like a toy. | [QLC+ forum](https://www.forum.qlcplus.org/viewtopic.php?p=68321), [Apple HIG](https://developer.apple.com/design/human-interface-guidelines/accessibility) |

---

## 3. Completeness checklist for the main screen

P1 = phase 1 (programming, cue lists, busking pages). Later = not phase 1.

| Zone / component | Titan source | P1 / Later | Notes |
|---|---|---|---|
| Workspace bar (Groups+Palettes, Fixtures, Attributes, Playbacks, Patch, Macros, Show) | Titan workspaces column | **P1** (Pixelmapper, Keystone, Shapes = Later) | Show "later" items disabled, or hide them. Never show empty fake screens. |
| Groups window | Groups window | **P1** | Number top-left, legend, fixture count. |
| Fixtures window (fixture buttons) | Fixture window | **P1** | Selected (blue), in programmer (red strip), parked, selection order. |
| Colours / Positions / Gobos & Beams palette windows | 2×2 grid | **P1** | Number TL, G/S/N TR, picture or colour legend, IPCGBESFX mask strip at the bottom, active-in-programmer state, greyed when not relevant. |
| Attribute editor (encoder display) | wheels plus IPCGBESFX tabs | **P1** | Tabs: active, available and has-values states. Three wheels with name, value and roller. Mirrors the BCF2000 encoder bank. |
| Colour picker (HS field, RGB/CMY, gel book) | Colour picker window | **P1** | Applies across colour systems. Can open in the attribute area. |
| Programmer summary (what is in the programmer) | Programmer / channel grid | **P1** | Red = programmer. Count of fixtures and attributes. Clear button nearby. |
| Function buttons: Record, Update, Edit, Delete, Copy, Move, Include, Release, Locate, Off, Fan, Options, Latch, Blind, Undo | Function column | **P1** (Select If, Unfold, Shape, Disk = Later or secondary) | Fixed order, matching Titan. |
| Keypad: Fixture, Group, Palette, Macro, Thro, And, @, Back, Avo, Set, Clear, Exit, Enter, Fix ±1, Odd/Even, Highlight, Locate, Full | keypad | **P1** | Collapsible on a laptop. The Mac keyboard covers numbers. |
| Command line / prompt (with undo history) | prompt area | **P1** | Always visible, one line, monospace syntax echo. |
| Softkeys A–G (context menu) | softkeys | **P1** | Context-sensitive and fixed in place. Titan users expect them on the right. |
| Pages column + page indicator | Pages | **P1** | `faders_per_page` is configuration. |
| Playback faders (legend/picture, level, Swop, Flash, select) | 10 faders | **P1** | Count = `faders_per_page` (default 8 for the BCF2000). Show cue number and running state on the handle. Halo colour mirrors to the controller where possible. |
| Cue-list controls: Go, Back/Stop, Prev/Next step, current & next cue, live/next time, fade progress | cue-list area | **P1** | The biggest single target on screen. Go is never hidden by a window. |
| Grand Master + Blackout (DBO) | master | **P1** | Always visible, with a strong state when the GM is below full or DBO is active. |
| Status strip: output OK/held, DMX device, daemon link, show name, autosave, clock | — | **P1** | Show-proof rule: surface errors as warnings, never as modals. |
| Blind indicator | Blind | **P1** | Full-frame state. |
| Macros window | Macros | Later (button only in P1) | |
| Effects / Shapes / Key-frame shapes | Shapes | Later | |
| Pixelmapper, Keystone, Timecode, Timeline, Set List | | Later | |
| Visualiser / stage plot | Visualiser | Later (simple fixture map = nice to have) | Lightkey's live preview is the benchmark. |
| Patch / Show / System | Patch, Disk, System | **P1** as separate screens, not on the main screen | |

---

## 4. Visual direction: modern and tight, without the dashboard look

### What "modern" means in this industry right now
- **Flat, bolder, fewer lines.** Hog v5 removed gradients, superfluous lines and faux-3D buttons ([Hog v5 notes](https://www.ushio-pro.com/Portals/0/images/entertainment/data/hog_release/Hog_v5-0-0-937_releasenotes_JP.pdf)). This is flat, but not soft: no big radii, no drop shadows, no glassmorphism.
- **Colour carries data, not decoration.** grandMA3's UI is mostly neutral grey; colour appears only for state (programmer red, selection yellow, tracked magenta, preset cyan) ([MA3 system colours](https://help.malighting.com/grandMA3/2.5/HTML/ws_colors_system.html)).
- **Dense single-window layouts** (the Ableton Live comparison). Users praise Live's flat, simple-colour, single-window GUI as "less busy", with secondary functions in context menus and hotkeys rather than chrome ([Steinberg forum](https://forums.steinberg.net/t/why-does-cubase-15-pro-look-like-an-old-video-game/1016808?page=5), [Tape Op](https://tapeop.com/reviews/gear/46/live-4)). This is the right reference for "strak".

### Grid and density
- One base unit (for example 4 pt), with buttons on a fixed square grid. Recommended cell: 56 × 56 pt for palette and group buttons, 44 pt minimum for anything touchable. Gaps of 2–4 pt, never 12–16 pt "card" spacing.
- Windows are divided by 1 px hairlines in a mid-dark grey, not by floating rounded cards. Corner radius is 2–3 pt at most, and 0 is fine.
- Zones are fixed and flush: edge to edge, no outer margins, no empty "breathing room". Every pixel shows show data.

### Typography
- Use a condensed sans for legends: SF Pro Condensed or Compressed via SwiftUI `Font.Width` *(Apple API; SF Pro ships in four widths per [Apple Fonts](https://developer.apple.com/fonts/))*. Use tabular, monospaced digits for every number (fader levels, cue numbers, DMX values) so values do not jitter while fading.
- Three sizes only: about 10–11 pt for metadata (palette number, G/S/N, mask letters), 12–13 pt for legends, and 15–20 pt for the cue number, Go and command line. Uppercase with tracking for tags (G/S/N, IPCGBESFX). Never bold everything.
- The command line uses a monospace face (SF Mono).

### Colour (proposal, to verify at the venue)
- **Neutrals:** background around #121314 (not #000), window surface one step lighter, hairlines around #2A2C2F, primary text around #D8DADC (not pure white, to reduce glare), secondary text around #8A8E93.
- **Reserved state colours (never decorative):**
  - Red (#E5484D-ish): programmer and manual values, unstored changes, the Record-armed state.
  - Blue: selected fixture or group (Titan convention).
  - Amber/yellow: active-but-fading (release fade) and warnings.
  - Green: a running playback, Go armed, output OK.
  - Magenta: tracked values (cue views later).
  - Cyan: value comes from a palette.
- **Attribute-group colours** for the IPCGBESFX strip and the encoder tabs. Pick one fixed hue per group and use it only for tags, the mask strip and the active-tab underline. Neither Titan nor MA documents a fixed per-group palette, so this is our own convention. Keep it muted (around 60% saturation) so it never competes with the red, blue and green state colours.
- **Content colour:** colour-palette legends show the real colour as a swatch strip or outline (the gel-outline lesson) plus the name. They are not full-bleed saturated fills.
- **Day theme:** same layout with bolder contrast, as in MA's defaultDAYLIGHT.

### Button states (one system for every button)
- **Empty:** hairline outline only, no fill, faint number.
- **Recorded:** surface fill, legend, metadata.
- **Selected:** 2 pt blue inner border plus a blue number.
- **Active / in programmer:** red top strip (3 pt) or red inner border. Fill unchanged, so the legend stays readable.
- **Running (playback):** green strip plus a progress bar along the bottom edge.
- **Not relevant to the selection:** 40% opacity.
- **Pressed / flash:** a momentary invert.
- **Halo:** a user colour as a 2 pt outer ring, separate from the state colours.

State is shown with borders and strips, never by swapping the whole fill. Fills stay neutral so many buttons in many states remain legible in the dark.

### Palette iconography
- Colour: a real swatch, or a split swatch for multi-colour palettes.
- Gobo: a monochrome gobo glyph, in a white line style.
- Position: a tiny stage-plot glyph (beam angle and dot).
- Beam: an iris or zoom glyph.
- Group: a fixture-count badge.
- Use one line weight (1.25 pt) and monochrome glyphs, so that the only colour on a palette button is the colour palette's own colour.

### Anti-patterns (the reasons iteration 1 was rejected)
- Rounded dark "cards" with shadows and generous padding: this is a SaaS-dashboard idiom that wastes density.
- Accent colour (purple, teal) used for branding or decoration. On a console every colour must mean a state.
- Large KPI-style numbers, charts, "status tiles", gradients, glows, glassmorphism.
- Oversized touch-first widgets (QLC+ 5).
- Windows that resize or reflow when content changes, and floating windows the user has to arrange (Titan's own pain).
- Several ways to do one thing, and hidden functions behind hover.
- Modal dialogs during playback, and error toasts that cover Go.
- Generic SF Symbols where a lighting-specific glyph is expected.
- Invented terminology. Use Titan's words (Swop, Flash, Latch, Off, Include, Avo).

---

## 5. Mac-specific notes
- **Native-Mac gap.** Titan is Windows-only and unsupported under Parallels on Apple silicon ([Avolites forum](https://forum.avolites.com/viewtopic.php?p=25044)). grandMA3 onPC runs natively on macOS 14+ ([MA3 onPC macOS](https://help.malighting.com/grandMA3/2.5/HTML/onpc_system_requirements.html)). Lightkey shows that a Mac-native app gets credit for full-screen use, gestures, autosave and unlimited undo ([lightkeyapp.com](https://lightkeyapp.com/)).
- **Full-screen "console mode" vs window.** Default to a full-screen single window (Lightkey and MA onPC both do this), with a normal windowed mode for editing patch and show files. In console mode, hide the menu bar and Dock and block Cmd-Q with a confirmation (unsourced recommendation).
- **Keyboard:**
  - Map the Titan keypad onto the Mac keyboard: digits, `-` = Thro, `+` = And, `@`, Enter, Esc = Exit, Backspace = Back/Clear. Space = Go only while console mode is armed.
  - Add an MA-style "shortcuts on" toggle with a visible state, which turns off automatically in text fields ([MA3 shortcuts](https://help.malighting.com/grandMA3/2.5/HTML/do_shortcuts_keyboard.html)).
  - F-keys for the workspace buttons.
- **Trackpad and mouse:**
  - Scroll over an encoder or fader to change its value (fine with Option, coarse with Shift, like MA's MA-key resolution and press-to-accelerate behaviour, [MA3 moving lights](https://help.malighting.com/grandMA3/2.5/HTML/qsg_control_moving.html)).
  - Click-drag on a fader.
  - Right-click opens the Titan context or softkey menu for that button.
  - Avoid hover-only affordances, because they do not exist on a touchscreen.
- **Touchscreen:** macOS has no native multitouch for external touchscreens, so treat touch as single-pointer clicks. That is a reason for the 44 pt floor, not for oversizing.
- **BCF2000:** it has no scribble strips. The screen is its label, as BCFView and mcpdisp do on Windows and Linux ([mcpdisp](https://github.com/ovenwerks/mcpdisp), [gearspace](https://gearspace.com/threads/behringer-controller.579454/)). The on-screen fader bank must sit in the same order as the 8 hardware faders, with the active bank and page highlighted, and the 8 encoders mirrored in the attribute area.
- **Dark Mode:** follow the system appearance only outside console mode. Console mode always uses the app's own dark or day theme.

---

## Design brief for iteration 2

1. **Layout skeleton (one full-screen window, no floating windows).** Left: a slim workspace rail (icons plus short labels). Centre: a fixed 2×2 window grid. Right: function-button column, then softkeys A–G. Bottom: pages column, `faders_per_page` playback strips, the cue-list/Go block, and the Grand Master and DBO. Top: a 24 pt status strip.
2. **Fixed zones that never reflow.** Window content scrolls inside its own frame. Grids have fixed rows and columns, so button N always sits in the same spot. Workspace presets swap the content of the 2×2 grid; they never move the bars.
3. **One attribute bar above the faders** in Hog v5 "Design Toolbar" style. It holds the IPCGBESFX tabs, the three wheels (name, value, roller), the command line with undo history, and the programmer count with Clear. The BCF2000 encoders map 1:1 to what this bar shows.
4. **Square button grid.** 56 pt palette and group cells, 2–4 pt gaps, 1 px hairlines, radius 0–3 pt. No cards, shadows or gradients.
5. **Palette button anatomy (exact Titan).** Number top-left (10 pt, tabular), G/S/N tag top-right, legend in the centre (condensed, 12 pt, two lines max), picture or swatch or glyph, and a 9-slot IPCGBESFX mask strip along the bottom with the contained groups lit.
6. **One state system for all buttons** (section 4): empty outline, recorded fill, blue = selected, red strip = in programmer, green strip plus progress = running, amber = release fading, 40% = not relevant, outer ring = user halo.
7. **Reserved colours.** Red = programmer or unstored, blue = selection, green = running or OK, amber = fading or warning, magenta = tracked, cyan = from palette. No other accent colours anywhere.
8. **Neutral palette** #121314 / #1A1C1E / hairline #2A2C2F / text #D8DADC / secondary #8A8E93. Add a day theme with higher contrast. Validate both at the church with stage light on.
9. **Type.** SF Pro Condensed for legends, SF Pro Text for UI, SF Mono for the command line. Tabular digits everywhere. Three sizes (10–11 / 12–13 / 15–20 pt). Uppercase tags.
10. **Playback strip.** Legend or picture on top (halo colour as a top bar), cue number and next cue, a vertical level bar with a numeric %, then Swop above Flash. The selected playback gets a blue frame, a running one a green strip, a fading-out one amber. The strip count comes from `faders_per_page`, never a hard-coded 8 or 10.
11. **Cue-list/Go block.** The largest target on screen. The Go button shows the next cue number and name; Back/Stop sit next to it, with Prev/Next step, live and next time, and a fade-progress bar. Never covered by any window or toast.
12. **Programmer visibility.** The fixture and group buttons with programmer values get a red strip. A programmer summary ("12 fx · I P C") sits next to the command line. The Clear button glows red while the programmer is non-empty.
13. **Blind mode.** A 3 pt full-frame border in a dedicated colour plus a "BLIND" tag in the status strip. Impossible to miss.
14. **Show-proof status strip.** Output state (live, held, error) for each output, daemon link, DMX device, show name, autosave time, clock. Errors are inline warnings and never modals.
15. **Undo and safety.** A visible undo history behind the prompt. Destructive actions (Delete, overwrite on Record) need a two-step confirm through softkeys, not a modal. A lock or venue mode leaves only Go and the faders active.
16. **Colour picker as an attribute-bar mode:** HS field plus linked RGB/CMY/HSI faders plus a gel book, with a per-fixture output swatch row (the Hog "Colour Mix" idea).
17. **Function buttons and keypad** in Titan's order and with Titan's names. The keypad can collapse on a 13–14" screen because the Mac keyboard covers numbers. Softkeys stay put.
18. **Show phase-1 scope honestly.** Later features (Shapes, Pixelmapper, Keystone, Timecode) appear as disabled rail items or are left out. No mock content.
19. **Mac input.** Scroll-to-adjust on wheels and faders (Option fine, Shift coarse), right-click opens the context softkeys, a visible "keys live" toggle, Space = Go only when armed, F-keys = workspaces.
20. **BCF2000 bank mirror.** The current hardware bank is outlined on the playback strips, and the encoder bank is shown in the attribute bar, because the controller has no displays of its own.

---

## Sources
- Avolites manual: [Quick start](https://manual.avolites.com/docs/quick-start) · [Workspace windows](https://manual.avolites.com/docs/titan-basics/workspace-windows) · [Other parts of the touch screen](https://manual.avolites.com/docs/titan-basics/other-parts-of-the-touch-screen) · [Creating palettes](https://manual.avolites.com/docs/palettes/creating-palettes) · [Using palettes](https://manual.avolites.com/docs/palettes/using-palettes) · [Controlling fixtures](https://manual.avolites.com/docs/quick-start/controlling-fixtures) · [Titan Mobile](https://manual.avolites.com/docs/about-the-consoles/titan-mobile)
- [Titan v17 release notes (PDF)](https://www.avolites.com/Portals/0/Downloads/ReleaseNotes/TitanSuite/TitanReleaseNotesV17.0.pdf) · [Titan 18.0 summary](https://audioeffetti.com/it/blog/avolites-titan-18-0-upgrades-new-features-and-enhancements-n170851) · [Titan v14, LSi](https://lsionline.com/news/avolites-releases-v14-titan-lighting-software) · [Titan v15, Blooloop](https://blooloop.com/technology/news/avolites-titan-v15/)
- [Avolites forum: Parallels not supported](https://forum.avolites.com/viewtopic.php?p=25044) · [Parallels forum: Avolites T2](https://forum.parallels.com/threads/avolites-t2.359715/)
- grandMA3 manual: [First view](https://help.malighting.com/grandMA3/2.5/HTML/qsg_first_view.html) · [Display configuration](https://help.malighting.com/grandMA3/2.5/HTML/ws_ui_display_configuration.html) · [System colours](https://help.malighting.com/grandMA3/2.5/HTML/ws_colors_system.html) · [Markers](https://help.malighting.com/grandMA3/2.5/HTML/ws_colors_markers.html) · [Colour themes](https://help.malighting.com/grandMA3/2.4/HTML/ws_colors_color_theme.html) · [Feature group bar](https://help.malighting.com/grandMA3/2.5/HTML/ws_eb_feature_group_control_bar.html) · [Keyboard shortcuts](https://help.malighting.com/grandMA3/2.5/HTML/do_shortcuts_keyboard.html) · [Colour picker](https://help.malighting.com/grandMA3/2.5/HTML/operate_color_picker.html) · [Control moving lights](https://help.malighting.com/grandMA3/2.5/HTML/qsg_control_moving.html) · [onPC requirements](https://help.malighting.com/grandMA3/2.5/HTML/onpc_system_requirements.html)
- [ALIA: Rohan Thornton on grandMA3](https://alia.com.au/rohan-thornton-chooses-an-ma-lighting-grandma3-onpc-command-wing-xt) · [fpcevents: grandMA3 vs Avolites](https://fpcevents.com/blog/grandma3-vs-avolites) · [ETC community: Hog over GrandMA2](https://community.etcconnect.com/control_consoles/legacy-consoles/f/legacy---wholehog-3/49656/hog-over-grandma2/159036)
- ETC: [Channel colour meanings](https://support.etcconnect.com/ETC/Consoles/Eos_Family/Software_and_Programming/Channels_Are_Stuck_On_and_Have_a_Colored_Value) · [Eos display conventions (PDF)](https://www.etcconnect.com/uploadedFiles/Main_Site/Content/Support/Articles/Consoles/Eos/Eos-Ion%20Displays%20Conventions.pdf) · [Apex, LSi](https://lsionline.co.uk/news/etc-extends-eos-range-with-apex-console) · [Apex, PLSN](https://plsn.com/?p=353115) · [Eos v3.0, ALIA](https://alia.com.au/?p=41850) · [Eos v3.3](https://www.etcconnect.com/About/News/ETC-Announces-Eos-v3-3-Software-with-Expansion-Processing-and-Much-More.aspx) · [Nomad look and feel](https://community.etcconnect.com/control_consoles/eos-family-consoles/f/eos-family/15066/what-is-the-correct-pc-program/58833) · [Magic sheets](https://community.etcconnect.com/control_consoles/eos-family-consoles/f/eos-family/12208/share-your-magic-sheets/48178) · [Magic sheet colour mismatch](https://community.etcconnect.com/control_consoles/eos-family-consoles/f/eos-family/29905/magic-sheet-colors-display-wrongly) · [Live/Blind switching complaint](https://community.etcconnect.com/control_consoles/eos-family-consoles/f/eos-family/15325/feature-request-bug-report-question-please-stop-automatically-switching-between-live-and-blind-and-changing-display-setup-archive)
- Hog: [v5.0.1, ALPD](https://www.thealpd.org.uk/news/313846/etc-releases-hog-v501-software) · [v5.0.0 release notes (JP)](https://www.ushio-pro.com/Portals/0/images/entertainment/data/hog_release/Hog_v5-0-0-937_releasenotes_JP.pdf) · [Hog 4 v4.0, ETC](https://www.etcconnect.com/About/News/ETC-Announces-Major-Update-to-Hog-4-Operating-System-with-v4-0.aspx) · [Hog 4 PC issues](https://community.etcconnect.com/control_consoles/hog-lighting-controls/f/hog-os-software-discussion/41273/a-few-hog-4-pc-issues-and-questions)
- [MagicQ layout](https://docs.chamsys.co.uk/magicq/manual/console_layout.html) · [MagicQ PC intro, Onstage Lighting](https://www.onstagelighting.co.uk/training-tutorials/magicq-pc-intro/) · [MQ70 reviews, Thomann](https://www.thomannmusic.com/chamsys_magicq_mq70_reviews.htm) · [Onyx 4.6, ALIA](https://alia.com.au/obsidian-releases-onyx-version-4-6)
- [Lightkey](https://lightkeyapp.com/) · [Lightkey review, learnstagelighting](https://www.learnstagelighting.com/blog/should-i-use-lightkey-full-lightkey-app-review%2F) · [Luminair, ControlBooth](https://www.controlbooth.com/threads/luminair-ipad-app.46162/latest) · [QLC+ 5 beta feedback](https://www.forum.qlcplus.org/viewtopic.php?p=68321)
- [Onstage Lighting: lighting control UI](https://www.onstagelighting.co.uk/lighting-equipment/lighting-control-ui/) · [ALIA LightShark review](https://alia.com.au/review-lightshark-ls-1) · [ControlBooth: considering Avolites](https://www.controlbooth.com/threads/considering-a-new-avolites-console.39529/latest)
- [MouseUX: interfaces in different lighting conditions](https://mouseux.com/design-and-characterization-of-interfaces-in-different-lighting-conditions/?lang=en) · [Material dark theme](https://m2.material.io/design/color/dark-theme.html) · [Apple HIG accessibility](https://developer.apple.com/design/human-interface-guidelines/accessibility) · [Apple Fonts](https://developer.apple.com/fonts/)
- [Steinberg forum on Ableton's flat GUI](https://forums.steinberg.net/t/why-does-cubase-15-pro-look-like-an-old-video-game/1016808?page=5) · [Tape Op: Live 4](https://tapeop.com/reviews/gear/46/live-4)
- [mcpdisp](https://github.com/ovenwerks/mcpdisp) · [Gearspace: BCF2000/BCFView](https://gearspace.com/threads/behringer-controller.579454/)
