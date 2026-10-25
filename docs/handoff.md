# Handoff: v0.2 local planner (2026-10-25)

This document records the state of Offhand at the end of the 2026-10-25 session. It lists what is broken, what is unverified, and the planned fixes. It is the starting point for the next session.

## Where the code is

- The v0.2 planner work is on the branch `feat/v0.2-planner`, in the worktree `~/projects/offhand-v02`. It has 27 commits on top of `main`. The last commit is `cb4c10d` (the BOS fix).
- **The branch is local only. It is not pushed to GitHub.** Push it before you work from another machine.
- The sub-branches `docs/v0.2-planner-adr`, `feat/v0.2-llama-generate` and `feat/v0.2-task-runner` are merged into `feat/v0.2-planner`. Delete them. A permission rule blocked the force delete in the last session.
- The scope is macOS only. Windows is out of scope for now.

## What v0.2 does

A user types a goal such as "open Calculator and type 12*3=". A local model proposes one step at a time. Each step is one of the existing T0 commands (`open`, `click`, `type`, `press`) or `done`/`fail`, constrained by a GBNF grammar.

| Part | Location |
|---|---|
| Planner loop, prompt, grammar, limits (12 steps, 3 stuck, 120 s) | `crates/offhand-core/src/planner.rs` |
| llama.cpp generator (llama-cpp-2 =0.1.160, Metal, greedy, 4096 context) | `crates/offhand-infer/src/llama_gen.rs` |
| Desktop app wiring, model load, debug logs | `app/src-tauri/src/actor/task.rs` |
| Host-terminal guard (stops the planner from typing into its own terminal) | `crates/offhand-macos/src/host.rs` |
| Irreversible-action check for autopilot | `crates/offhand-policy/src/lib.rs` (`is_irreversible`) |
| CLI entry: `offhand task "<goal>" --model <path>` (feature `planner`) | `crates/offhand-cli/src/main.rs` |
| ADR (status Proposed) | `docs/adr/0006-local-planner-tier.md` |

- **Model:** Liquid AI LFM2.5-2.6B Q4_K_M, at `~/models/offhand/LFM2.5-2.6B-Q4_K_M.gguf`. The SHA-256 is `02a8b7e17487d326e46d68ce0ba24211e1b80a14c4cd0597fa73c1cd697f52ed`. The license is LFM Open License 1.0 (not OSI, $10M revenue cap). Override the path with `OFFHAND_PLANNER_MODEL`.
- **Autopilot:** the app runs planner tasks with no confirmation prompts, as the user asked. Autopilot stops the task before an irreversible step (send, delete, remove, buy, pay, purchase, checkout, submit, password, empty trash, recycle bin). Steps that policy blocks stay blocked.

## Broken or unverified now

1. **The BOS fix is not checked end to end.** The planner produced garbage because llama.cpp's chatml template omits the BOS token, and LFM derails without it. Commit `cb4c10d` adds BOS at tokenize time. The reference `llama-completion` gave `{"action":"open","arg":"Calculator"}` for the same prompt. No goal ran in the app after the fix.
2. **The model must name elements as free text.** It writes a label such as "Search", and the label must match an on-screen element. A small model often names an element that does not exist.
3. **The model gets no feedback from its steps.** Every successful step reports `ok`. The model cannot see that a click opened a menu or did nothing.
4. **The screen read omits static text.** The model cannot read results, for example the Calculator display or a list of search results.
5. **One model call does both the planning and the next action.** This is too much for a 2.6B model.
6. **There are no per-app actions.** The model must click through every app. For example, Outlook has no `search` shortcut.
7. **There is no fixed goal list to measure success.** We cannot tell if a change helps.
8. **The host guard has gaps under tmux and ssh.** The guard checks the frontmost app and the ancestor process chain. It can miss a terminal behind tmux or an ssh session.
9. **The model file has no hash check at load time.**
10. **Model text that the app shows is not cleaned of bidi and C1 control characters.** `printable` replaces only control characters that Rust's `is_control` matches.
11. **ADR-0006 does not match the code.** Its description of "stuck" differs from the code: a malformed reply and a failed step now both count as stuck.
12. **Platform code sits in the CLI crate.** It should move behind `offhand-platform`.
13. **There is no latency benchmark for planner steps.**

## Planned fixes, in order

1. **Run three goals in the app (about 15 minutes).** Use "open Calculator and type 12*3=", "open Notes and type hello", and one Outlook goal. Read the `[offhand planner]` lines in the dev log. This shows how much of the failure the BOS bug caused.
2. **Make the model pick elements by number (about 2 hours).** Number the elements in the prompt. Generate the grammar per step so that it allows only numbers that exist, for example `click 7`. This fixes item 2 and is expected to give the largest gain.
3. **Report what each step changed (about 2 hours).** Return the new window title, the focused element, and the elements that appeared or disappeared. Add static text to the screen read. This fixes items 3 and 4.
4. **Split planning from acting (about 1.5 hours).** First make one call that writes the goal as a short checklist. Then make one call per step that chooses the action for the current checklist item (the Agent S manager/worker pattern). This fixes item 5.
5. **Add per-app actions (about 3 hours for the first apps).** Add an Outlook `search "<query>"` action that focuses the search box and types the query, for example `hasattachments:yes received:last week`. Use AppleScript for Mail, Finder, Calendar and Notes. UFO2 reports 6 to 8 points more success and up to 58% fewer steps with API-first actions. This fixes item 6.
6. **Compare Qwen3.5-4B with LFM (about 30 minutes).** Qwen3.5-4B has an Apache license. Run the same goals on both models and keep the model that completes more goals.
7. **Add a script that runs 10 fixed goals (about 30 minutes).** The script feeds the goals through `offhand task` and counts how many finish. Run it after each change. This fixes item 7. The user asked for no test suite, so keep this as a script.

The total is about 1.5 days. Items 8 to 13 come after these steps.

## Open decision: cloud mode

The user asked about a cloud mode: a cheap planner through OpenRouter (DeepSeek Flash, GLM) and a fast hosted vision model for clicks. The recommendation from this session:

- Put the planner in the cloud first. `LlmPlanner` takes a `Generator`, so an OpenRouter generator is about 2 to 3 hours of work. The grammar becomes a JSON schema.
- Keep the accessibility tree as the main way to act. Use a hosted vision model (UI-TARS or Qwen-VL family) only when the tree cannot find the target.
- Make cloud mode an explicit opt-in, and keep every step behind offhand-policy.

The cost is privacy. Screenshots and email contents leave the machine, and the project rule now says that inference stays local. **The user must decide two things:** whether screenshots can leave the machine, and what "Cloudflare Clev" means (it is not clear which host or model the user meant).

## Other open items

- **PR #8** (`fix/v0.1-overlay-clickthrough`, worktree `~/projects/offhand-fix-overlay`) needs review and merge.
- **Overlay colour:** the edge glow and the second agent cursor need a colour. `#6EA8FE` (blue) is proposed. There is no decision yet.
- **Dependabot PRs #1 to #4** need triage. #4 is a TypeScript major version (6 to 7).
- **Misleading "Native operation failed" message.** Change it to show the real cause.
- **`signingIdentity` in the Tauri config** needs a fix before a signed release build.
- **`app/src-tauri/permissions/autogenerated`** is untracked in both worktrees. Decide if it goes in `.gitignore`.
- **brew `llama.cpp` 0.7.0** was installed only to compare output. Uninstall it if it is not needed.

## Run and stop on this machine

- Start dev mode: `OFFHAND_DEV_MINUTES=60 offhand-dev ~/projects/offhand-v02`. The watchdog stops the app after the given number of minutes (default 10).
- Build with `CARGO_TARGET_DIR=~/projects/offhand/target CARGO_BUILD_JOBS=4 nice -n 10` to limit CPU and RAM.
- Kill switch: press Control + Option + Command + K (skhd), or run `offhand-kill`. It stops only Offhand processes. Do not quit Warp.
- No old `/Applications/Offhand.app` is installed. The v0.1 copy was moved to the Trash.
- Planner logs start with `[offhand planner]`: the model reply and its time, each step and its result, the final outcome, and model-load errors.

## Rules carried forward

- Inference stays local unless the user approves cloud mode.
- Every action goes through offhand-policy.
- Do not commit model weights.
- Never answer a confirmation for the user.
- Ask before force pushes, destructive commands, or anything that sends data off the machine.
