# Research: a local language model that builds show content from chat

Status: research only. No acceptance criteria, no code. Written 2026-10-25. Related: IDEA-17
(MCP server for cloud assistants, phase 2), IDEA-21 (paid tier), IDEA-25 (2D rig layout).

**The question.** Can CMDMX run a language model on the operator's Mac, offline, behind a chat
window, that turns requests such as "make the default colour palettes for all fixtures", "build
basic groups (all spots, all washes, odds/evens)" or "make a position palette with all movers
pointing at the stage" into show content, without ever hurting the DMX tick?

**Short answer.** Yes, for requests that are mostly *choosing and filling parameters* of
deterministic built-in commands. A 4B model at 4-bit with JSON-schema-constrained output does that
well on a 16 GB Mac; the model should not invent DMX values itself. Two gaps in the current
engine come first, and they are worth building even without a model: (1) palettes can only be
recorded *through the programmer*, which is a live merge source (PRG-13, PAL-27), so building
palettes today changes the stage; (2) there is no "apply many commands as one previewed, undoable
step".

Measured on the owner's Mac while writing this: Apple M4, 16 GB, macOS 26.6.2; the Command Line
Tools SDK ships `FoundationModels.framework` but **no `metal` shader compiler**. The church
laptop's chip, RAM and macOS version are unknown (open question).

---

## 1. Runtime

| Runtime | How it embeds | Licence | Size in the app | Fit for CMDMX |
|---|---|---|---|---|
| **llama.cpp `llama-server`** (child process, OpenAI-style HTTP on loopback) | Separate process the assistant starts; GGUF models; Metal backend; `response_format` / `json_schema` / GBNF grammars; tool calling via the model's chat template (`--jinja`) | MIT | Server binary in the tens of MB (guess; measure in the spike). Model downloaded on first use | **Best.** Own process (a C++ `abort()` or OOM cannot take the daemon down), grammar-constrained output, any open model, no Xcode needed (Metal source is embedded and compiled at run time; verify in the spike) |
| llama.cpp via Rust bindings (`llama-cpp-2`) | Linked into a Rust binary; Metal feature flag; grammar / JSON-schema sampler | MIT / bindings MIT-Apache | Same plus build time (cmake via `cc`) | Fine for a separate `cmdmx-assistant` binary; **never in `cmdmx-daemon`** (C++ panics are not caught by `catch_unwind`; breaks the show-proof rules) |
| MLX / mlx-swift (`mlx-swift-lm`) | Swift package in the app | MIT | Small runtime; model download | **Does not fit the toolchain:** "SwiftPM (command line) cannot build the Metal shaders"; needs `xcodebuild`, and the project uses Command Line Tools only. Faster than llama.cpp on short contexts, but no benefit at our sizes worth an Xcode dependency |
| Ollama | Separate install by the operator; HTTP API on loopback | MIT | Nothing in the app; Ollama is ~hundreds of MB plus models | Good for the **spike** and for power users. Not as the default: an extra install at the venue, its own updater, a background service. 0.19 moved to MLX on Apple Silicon (preview, >32 GB only); GGUF still goes to llama.cpp |
| **Apple Foundation Models** | Swift API (`LanguageModelSession`), tool protocol, guided generation (`@Generable`, constrained decoding). Model runs in a system process | Apple SDK, no model licence | **Zero**: the model is part of the OS | Attractive second option. Needs macOS 26+ on an Apple-Intelligence Mac (M1 or later) with Apple Intelligence switched on. ~3B parameters, 2-bit (Apple tech report 2025). Context **4,096 tokens** (InfoQ, 2026-03); WWDC26 shows `contextSize` 8,192 for the rebuilt model (macOS 27). Swift-only, so the tool loop lives in Swift, not Rust. The model changes with OS updates (behaviour not pinned). Dutch: check Apple Intelligence language list |

**Model shipping.** Do not put a 2.5–6 GB model in the app bundle or DMG. Download on first use
of the assistant (Hugging Face URL pinned in the app, SHA-256 checked, resumable), stored in
`~/Library/Application Support/CMDMX/models/`; after that everything is offline. For the church:
download at home before the laptop goes to the venue. Apple FM needs no download.

## 2. Models

All figures are 4-bit GGUF (Q4_K_M or similar). Tool-calling scores are third-party and change
monthly; BFCL v3 and v4 are not comparable. Speeds are **guesses** from community reports, to be
measured with `llama-bench` in the spike.

| Model | Params | Disk (Q4) | RAM incl. 16k context (guess) | Tool calling (source) | Licence | Speed guess, generation tok/s (M1 / M3 / M4) |
|---|---|---|---|---|---|---|
| **Qwen3.5-4B** (2026-03) | 4B dense | ~2.8 GB | ~4 GB | BFCL-v4 0.503, "marginal" alone; good with constrained output | Apache-2.0 | 25 / 40 / 45 |
| **Gemma 4 E4B** (2026-04) | ~4B effective | ~3–4 GB (guess) | ~5 GB | native function calling + JSON; BFCL-v4 mid-to-high 80s non-live (third-party) | Apache-2.0 (first Gemma under it) | 20 / 35 / 40 |
| **Phi-4-mini** | 3.8B | ~2.5 GB | ~3.5 GB | "safest single small pick" (d-central) | MIT | 25 / 40 / 45 |
| Qwen3.5-9B | 9B dense | ~6 GB | ~7.5 GB | BFCL-v4 0.661, "usable" | Apache-2.0 | 12 / 20 / 25 |
| Granite 4.1 8B | 8B | ~5 GB | ~6.5 GB | BFCL-v3 68.3 % | Apache-2.0 | 13 / 22 / 26 |
| Apple on-device model | ~3B, 2-bit | 0 (in the OS) | system-managed | tool protocol + guided generation; "better at tool calling" (WWDC26) | Apple | not published |

Below ~4B, general tool calling is unreliable (2B BFCL-v4 0.436). All candidates above are
Apache-2.0 or MIT, so they suit an open-source project *and* a paid tier (IDEA-21); avoid Gemma 2/3
terms, Llama community licences and CC-BY-NC models (Command-R).

**Hardware.** 16 GB (baseline): any 4B comfortably, an 8–9B possible but it competes with the
app; recommend 4B. 8 GB: a 4B at Q4 with a short context (~4 GB), or Apple FM; tight next to
the app, and macOS swapping is the real risk (guess: usable, but test on the church laptop).
Prompt processing matters more than generation here: a 3k-token context is a few seconds on an
M1 (guess), and llama-server's prompt cache makes follow-up turns fast.

## 3. Reliability for this domain

**Context size.** The church demo show (`tests/fixtures/shows/church-demo.cmdmx.json`, 38
fixtures, 5 profiles) is 14 KB pretty, **7 KB minified ≈ 2–2.5k tokens** (guess at ~3 chars per
token for JSON); patch is 3 KB of it, profiles 1.3 KB. A real 40-fixture rig with library movers
(30+ attributes, wheel slots per profile) is more like 10–25k tokens raw (guess). That fits a
32k-context 4B model, not Apple FM's 4k.

**What the model should see: a compact summary, not the snapshot.** One line per fixture *type*
and the facts a request needs, built by deterministic code (the daemon or the assistant):

```
types: Wash x4 (101-104) I:Dimmer P:Pan16,Tilt C:RGBW B:Zoom | Par RGBW x8 (201-208) I C:RGBW |
       Spot x6 (301-306) I | Mover x4 (401-404) P:Pan,Tilt C:wheel[Red,Blue,Green,Yellow,Open] G:gobo[...]
groups: 1 All, 2 Washes, 3 Pars, 4 Front spots   palettes: Colours 1-3 used; Positions empty
```

That is a few hundred tokens for 40 fixtures, fits every runtime including Apple FM, and leaves
the model nothing to misread. Raw values (DMX ranges, channel numbers) never go to the model.

**Tools vs. plan vs. grammar.** Recommended combination:
- **Model output = one plan**, a short JSON list of *high-level* calls, validated against a JSON
  Schema. With llama.cpp the same schema is enforced during decoding (`json_schema` → grammar),
  so the output always parses; with Apple FM, `@Generable` does the same. Fixture ids and group
  names can be put in the schema as `enum`s from the summary, so the model cannot name a fixture
  that does not exist.
- **Not one tool call per palette.** Twenty round trips through a 4B model multiply the error
  rate and the latency, and each low-level IPC command is a separate undo step.
- A **read-only tool or two** (e.g. "list profile attributes of type X") is fine for questions,
  but generation should be plan-in-one-shot.
- Grammar guarantees syntax, not meaning: the daemon still validates every call and refuses with
  warnings as today (IPC-06).

## 4. What should be deterministic code (wizards)

The example requests are better as built-in engine commands with parameters; the model only
picks the wizard and fills the parameters. Each wizard is useful as a plain button too.

| Request | Wizard (engine, deterministic) | What the model fills in |
|---|---|---|
| "Default colour palettes for all fixtures" | `wizard.colour_palettes`: a fixed colour list (white, warm, red, orange, yellow, green, cyan, blue, magenta, …) resolved per fixture: RGB/RGBW/CMY by the existing colour maths (PRG-30/34), colour wheels by slot-name match (`Red` ↔ red), skip fixtures without a match; one palette per colour, shared kind (PAL-04) | fixtures (all / group), which colours, start cell |
| "Basic groups: all spots, all washes, odds/evens" | `wizard.groups`: one group per profile/type, per attribute class (has P = movers, has C = colour), and odd/even of each (PRG-05) | which groupings, names |
| "Position palette, all movers pointing at the stage" | `wizard.position_palette` with presets (centre, stage floor, audience, straight down) as Pan/Tilt fractions per profile's range; real aiming at a point needs fixture positions and orientation (IDEA-25 and beyond): **not possible honestly today** | preset, fixtures, legend |
| "Make a cue for the sermon: warm front, blue wash" | combination of existing palettes into a cue | which palettes on which groups, levels (later; needs blind recording) |

Rule of thumb: if the answer is a function of the patch and profiles, it is a wizard. The
model's value is language ("odds and evens of the pars, and a warm look"), not lighting maths.

## 5. Safety and show-proofing

- **Blind by construction.** Today `palette.record` records from the programmer, and the
  programmer is in the live merge, so a model-built palette would light the rig. The assistant
  needs *direct edit commands* that write show data without the programmer: e.g.
  `palette.store {window, handle, legend, values: {fixture: {attribute: value}}}` and
  `group.store {number, name, items}`. These are edit-class commands (UNDO-04) and touch no output.
- **Preview, then one step.** A `plan.preview {commands}` runs the plan on a copy of the edit
  state and returns the diff ("creates groups 5–8, palettes Colours 4–12; changes no output");
  the chat window shows it; the operator taps Apply; `plan.apply` applies the same plan as **one
  undo step** (UNDO-03 already defines one confirmed action = one step). Refused if the show
  changed in between.
- **Never live without the operator.** Plans may contain only edit-class commands. Live-class
  commands (faders, Go, `palette.apply`) are not in the assistant's schema at all (phase 1).
- **Process isolation.** The model runs in its own process (llama-server, or the OS process for
  Apple FM), never in the daemon. The daemon's tick is CPU-only and small, so GPU use by the
  model does not compete with it directly; the real risks are memory pressure (swap) and heat.
  Mitigations: start the model only when the chat window opens and unload after idle;
  `taskpolicy`/background QoS for the assistant process; refuse to load a model that does not
  fit in free RAM; a test run that logs tick overruns while the model generates.
- **During a show** the assistant is a planning tool: no auto-apply, a clear "nothing changes
  until you press Apply" in the chat window. Whether it should be locked while playbacks are live
  is an owner decision.

## 6. One tool layer for the local model and IDEA-17

Yes, and that is the main reason to do it this way. The shared part is:
1. a **machine-readable command catalogue**: a JSON Schema per IPC command (from `ipc.md`, with
   its undo class), checked by a test so no command kind is missing (as UNDO-04 does for classes);
2. the **compact show summary** (§3), as an IPC request;
3. the **wizards** and **`plan.preview` / `plan.apply`** in the engine.

On top of that, two thin clients of the same WebSocket IPC (ADR-002): `cmdmx-assistant` (local
model via llama-server, chat window in the app) and the MCP server (stdio, for Claude). Both
expose the same tools and the same preview/apply flow; a cloud model just handles harder
requests. The Apple FM route can reuse the catalogue and the plan schema but its loop is Swift.

## 7. Verdict

**Feasible**, for parameter-filling requests on top of wizards ("make colour palettes for the
pars", "odds and evens of the washes", "a position palette with the movers centre stage"), on a
16 GB Apple Silicon Mac with a 4B model; probably on 8 GB with care. **Not** sensible for
requests that need geometry or taste the model cannot see (true aiming, designing a whole
service) without more show data.

**Recommended architecture.** Engine gains wizards, blind store commands and plan preview/apply
as one undo step; a separate `cmdmx-assistant` process (IPC client, same tool catalogue as the
future MCP server) runs llama-server with Qwen3.5-4B or Gemma 4 E4B and JSON-schema-constrained
plans, and the app's chat window shows the preview and the Apply button.

**Rough size (each its own spec and PR):**

| Task | Size |
|---|---|
| Command catalogue as JSON Schema + completeness test (shared with IDEA-17) | M |
| `palette.store` / `group.store` (blind writes, edit class) | M |
| `plan.preview` / `plan.apply` (diff on a copy, one undo step) | M–L |
| Wizards: groups S, colour palettes M, position presets S | S + M + S |
| Compact show summary over IPC | S |
| `cmdmx-assistant` + llama-server lifecycle + model download | M |
| Chat window in the app (zone in UI v0.1) | M |
| MCP server on the same layer (IDEA-17) | S once the above exist |

Wizards, blind stores and preview/apply are worth building first even if the assistant is
never shipped; the model is a thin optional layer on them.

**Spike (1–2 days, throwaway code in a scratch folder, no engine change):**
1. Start the daemon on a temp `--show-dir` with the church demo show, output on Art-Net only.
2. Run `llama-server` (prebuilt macOS arm64 release) with Qwen3.5-4B, Gemma 4 E4B and
   Phi-4-mini at Q4; `llama-bench` each on the owner's M4 (and the church laptop if possible).
3. A small script builds the compact summary from the real IPC snapshot and asks 20 prompts
   (Dutch and English) for a plan constrained by a JSON Schema of today's commands
   (`programmer.command` selections + `group.record`; for palettes `programmer.select`/`wheel`
   + `palette.record`, then `programmer.clear`), and applies valid plans over the real IPC.
4. Repeat once with Apple FM through a 50-line Swift command-line tool, to compare.
5. Measure: valid-plan rate, correct-plan rate (scored by hand against expected plans), first
   answer latency, peak RAM, and DMX tick overruns/jitter while generating.

Pass bar (proposal): a 4B model gets ≥ 18/20 plans right, answers in < 10 s on the M4, no tick
overrun. The spike will also show concretely that palette building through the programmer
changes the Art-Net output, which is the case for `palette.store`.

---

## Sources

- d-central, *Local LLM agent capability* (BFCL scores per model, reviewed 2026-07-18): https://d-central.tech/local-llm-agent-capability/
- Artificial Analysis, *Qwen3.5 small models* (sizes, licence, 4-bit memory): https://artificialanalysis.ai/articles/qwen3-5-small-models
- Qwen3.5-4B GGUF (Q4_K_M ~2.78 GB): https://huggingface.co/munimentai/Qwen3.5-4B-GGUF
- The Decoder, *Gemma 4 under Apache 2.0*: https://the-decoder.com/googles-gemma-4-is-now-available-with-apache-2-0-licensing-for-the-first-time/
- Gemma 4 E4B GGUF: https://huggingface.co/npario/gemma-4-E4B-it-GGUF
- Apple, WWDC26 session 241 (rebuilt on-device model, `contextSize`, tool calling): https://developer.apple.com/videos/play/wwdc2026/241/
- Apple, WWDC25 *Meet the Foundation Models framework* (guided generation, tools, requirements): https://developer.apple.com/videos/play/wwdc2025/286
- InfoQ, *Foundation Models context window* (4,096 tokens, token APIs): https://infoq.com/news/2026/03/apple-foundation-models-context
- mlx-swift README (SwiftPM cannot build Metal shaders): https://github.com/ml-explore/mlx-swift
- Ollama, *powered by MLX on Apple Silicon (preview)*: https://registry.ollama.ai/blog/mlx
- llama.cpp structured output via `json_schema` / grammars (Liquid AI docs): https://docs.liquid.ai/deployment/on-device/llama-cpp/structured-output
- InsiderLLM, structured output with local LLMs: https://insiderllm.com/guides/structured-output-local-llms/
- llama.cpp vs MLX on Apple Silicon (2026): https://www.local-llm.net/compare/llama-cpp-vs-mlx/
- llama.cpp repository (MIT, server, grammars): https://github.com/ggml-org/llama.cpp
- `llama-cpp-2` crate: https://docs.rs/llama-cpp-2
