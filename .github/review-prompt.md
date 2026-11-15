<!--
The review prompt of .github/workflows/review.yml (specs/modules/autoreview.md, ARV-01..03).
Steps 1..8 below are the command of the code-review plugin, anthropics/claude-plugins-official commit 9780cf4
(plugins/code-review/commands/code-review.md, v1.0.1, Apache License 2.0), with two changes:
- its first step is dropped: an agent that stopped the review for a PR that is closed, a draft, "automated or
  trivial", or one Claude had commented on before. The workflow never runs on a closed PR or a draft, every PR here is written by
  Claude, and every push is reviewed; that stop left #305..#307 not reviewed (ARV-01, amended 2026-11-15);
- this project's severity, verdict and structured output are added (ARV-02, ARV-03).
The review launches two agents to prepare and four to review; scripts/risk-gate.py reads fewer as not reviewed.
The workflow appends the pull request to review after this file.
-->
Provide a code review for the given pull request, and post its issues as inline comments (the plugin's `--comment`).

Every PR in this repository is written by Claude and is reviewed in full at every push, including PRs you have
reviewed before. Do not stop early for any reason other than a tool that fails.

**Agent assumptions (applies to all agents and subagents):**
- All tools are functional and will work without error. Do not test tools or make exploratory calls. Make sure this is clear to every subagent that is launched.
- Only call a tool if it is required to complete the task. Every tool call should have a clear purpose.

To do this, follow these steps precisely, launching every agent with the Agent tool:

1. Launch a haiku agent to return a list of file paths (not their contents) for all relevant CLAUDE.md files including:
   - The root CLAUDE.md file, if it exists
   - Any CLAUDE.md files in directories containing files modified by the pull request

2. Launch a sonnet agent to view the pull request and return a summary of the changes

3. Launch 4 agents in parallel to independently review the changes. Each agent should return the list of issues, where each issue includes a description and the reason it was flagged (e.g. "CLAUDE.md adherence", "bug"). The agents should do the following:

   Agents 1 + 2: CLAUDE.md compliance sonnet agents
   Audit changes for CLAUDE.md compliance in parallel. Note: When evaluating CLAUDE.md compliance for a file, you should only consider CLAUDE.md files that share a file path with the file or parents.

   Agent 3: Opus bug agent (parallel subagent with agent 4)
   Scan for obvious bugs. Focus only on the diff itself without reading extra context. Flag only significant bugs; ignore nitpicks and likely false positives. Do not flag issues that you cannot validate without looking at context outside of the git diff.

   Agent 4: Opus bug agent (parallel subagent with agent 3)
   Look for problems that exist in the introduced code. This could be security issues, incorrect logic, etc. Only look for issues that fall within the changed code.

   **CRITICAL: We only want HIGH SIGNAL issues.** Flag issues where:
   - The code will fail to compile or parse (syntax errors, type errors, missing imports, unresolved references)
   - The code will definitely produce wrong results regardless of inputs (clear logic errors)
   - Clear, unambiguous CLAUDE.md violations where you can quote the exact rule being broken

   Do NOT flag:
   - Code style or quality concerns
   - Potential issues that depend on specific inputs or state
   - Subjective suggestions or improvements

   If you are not certain an issue is real, do not flag it. False positives erode trust and waste reviewer time.

   In addition to the above, each subagent should be told the PR title and description. This will help provide context regarding the author's intent.

   In this project, also check the changes against the acceptance criteria the PR title and body name (ids like
   ENG-01 or ARV-05, defined in `specs/modules/*.md`), and against the show-proof rules and architecture constraints
   of CLAUDE.md. Do not report style.

4. For each issue found in the previous step by agents 3 and 4, launch parallel subagents to validate the issue. These subagents should get the PR title and description along with a description of the issue. The agent's job is to review the issue to validate that the stated issue is truly an issue with high confidence. For example, if an issue such as "variable is not defined" was flagged, the subagent's job would be to validate that is actually true in the code. Another example would be CLAUDE.md issues. The agent should validate that the CLAUDE.md rule that was violated is scoped for this file and is actually violated. Use Opus subagents for bugs and logic issues, and sonnet agents for CLAUDE.md violations.

5. Filter out any issues that were not validated in step 4. This step will give us our list of high signal issues for our review.

   Give every issue a severity:
   - blocker: breaks a show-proof rule, can stop or corrupt output, loses show data, or a criterion the PR claims
     is not met at all.
   - major: wrong behaviour a user or the harness would hit.
   - minor: a small defect, a criterion partly met.

   And a verdict: confirmed when the validation step checked it against the code, plausible when it was flagged but
   validation could not settle it. Post inline comments only for confirmed issues.

6. Output a summary of the review findings to the terminal:
   - If issues were found, list each issue with a brief description.
   - If no issues were found, state: "No issues found. Checked for bugs and CLAUDE.md compliance."

   If NO issues were found, post a summary comment using `gh pr comment` and go to the last step.

   If issues were found, continue to step 7.

7. Create a list of all comments that you plan on leaving. This is only for you to make sure you are comfortable with the comments. Do not post this list anywhere.

8. Post inline comments for each confirmed issue using `mcp__github_inline_comment__create_inline_comment` with `confirmed: true`. For each comment:
   - Provide a brief description of the issue
   - For small, self-contained fixes, include a committable suggestion block
   - For larger fixes (6+ lines, structural changes, or changes spanning multiple locations), describe the issue and suggested fix without a suggestion block
   - Never post a committable suggestion UNLESS committing the suggestion fixes the issue entirely. If follow up steps are required, do not leave a committable suggestion.

   **IMPORTANT: Only post ONE comment per unique issue. Do not post duplicate comments.**

Last step: return every issue, confirmed or plausible, in the structured output (file, line, severity, verdict,
summary); `findings: []` when there are none.

Use this list when evaluating issues in Steps 3 and 4 (these are false positives, do NOT flag):

- Pre-existing issues
- Something that appears to be a bug but is actually correct
- Pedantic nitpicks that a senior engineer would not flag
- Issues that a linter will catch (do not run the linter to verify)
- General code quality concerns (e.g., lack of test coverage, general security issues) unless explicitly required in CLAUDE.md
- Issues mentioned in CLAUDE.md but explicitly silenced in the code (e.g., via a lint ignore comment)

Notes:

- Use gh CLI to interact with GitHub (e.g., fetch pull requests, create comments). Do not use web fetch.
- Create a todo list before starting.
- You must cite and link each issue in inline comments (e.g., if referring to a CLAUDE.md, include a link to it).
- If no issues are found, post a comment with the following format:

---

## Code review

No issues found. Checked for bugs and CLAUDE.md compliance.

---

- When linking to code in inline comments, follow the following format precisely, otherwise the Markdown preview won't render correctly: https://github.com/anthropics/claude-code/blob/c21d3c10bc8e898b7ac1a2d745bdc9bc4e423afe/package.json#L10-L15
  - Requires full git sha
  - You must provide the full sha. Commands like `https://github.com/owner/repo/blob/$(git rev-parse HEAD)/foo/bar` will not work, since your comment will be directly rendered in Markdown.
  - Repo name must match the repo you're code reviewing
  - # sign after the file name
  - Line range format is L[start]-L[end]
  - Provide at least 1 line of context before and after, centered on the line you are commenting about (eg. if you are commenting about lines 5-6, you should link to `L4-7`)
