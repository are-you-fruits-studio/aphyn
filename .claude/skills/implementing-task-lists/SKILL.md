---
name: implementing-task-lists
description: >-
  Use when the user hands over a list or bulk of tasks to implement in the aphyn workspace (pasted
  in chat, a file path, or a section of todos.md), such as "implement these tasks", "do this list",
  "work through the Prioritized section". Invoke as /implementing-task-lists <tasks | file | section>.
---

# implementing-task-lists

Implement a list of tasks **one at a time, in the order written**, each delegated to its own subagent and
each landing as its own commit. You are the orchestrator: you clarify, brief, verify and commit. The
subagents write the code.

The three phases run strictly in sequence: **clarify everything → implement task by task → report**.
No code is written before the clarify phase ends.

## Phase 1: Clarify (before any implementation)

1. **Collect the list.** From the argument: pasted text, a file path, or a section of a file (e.g. the
   `Prioritized` section of `todos.md`). Number the tasks in their written order. Nested bullets belong
   to the task above them.
2. **Learn enough to ask good questions.** For each task, read the `//!` crate docs of the crates it
   touches (see `CLAUDE.md`) and skim the relevant code. Use an `Explore` subagent for wide searches.
   Questions informed by the code ("the hierarchy already has a right-click menu in X; reuse it or build a
   new one?") are far better than generic ones.
3. **Ask, in rounds, until nothing is open.** Use `AskUserQuestion` (up to 4 questions per call; call it
   as many times as needed). Cover for every task:
   - scope: what exactly is in, what is out
   - expected behavior / UX, edge cases
   - design choices the code does not decide (which approach, which API shape, where it lives)
   - dependencies between tasks, and whether the written order should change
   - what "done" looks like and how it can be checked (build, example to run, doctest)

   Follow-up answers often raise new questions: ask another round. There is no limit on the number of
   questions; the limit is "no remaining ambiguity".
4. **Confirm commits.** If the user did not invoke this skill by name, ask once whether every task
   should be committed separately on the current branch (`CLAUDE.md` allows git writes only on explicit
   request). Also ask about pre-existing uncommitted changes in files a task will likely touch.
5. **Write the task sheet** (keep it in your context, not in the repo): per task, its text, the agreed
   decisions, and done criteria. Show it to the user and get a final go-ahead.

## Phase 2: Implement, one task at a time

Keep a short **carry-forward notes** list: facts learned while implementing that later tasks need
(new types/functions and where they live, changed APIs, conventions discovered, pitfalls hit).

For each task, in order:

1. **Brief a fresh `general-purpose` subagent.** Never run two tasks in parallel. The brief contains,
   in this order:
   - **Task:** the task text and its agreed decisions and done criteria from the task sheet.
   - **Context from earlier tasks:** only the carry-forward notes this task actually needs, each a
     one-liner with file paths. Omit the section if none apply. Do not paste whole earlier reports,
     diffs, or the full Q&A.
   - **Where to start:** crates/files to read first, including their `//!` docs.
   - **Rules:** follow `CLAUDE.md` (comment policy, `cargo fmt`, keep `add_defult_modules_to`
     spelling, update a crate's `//!` docs when its behavior or public API changes); do not run any git
     write command; if an undecided design choice comes up, stop and return the question instead of
     choosing.
   - **Report back:** files changed; what was done; any deviation from the brief; facts a later task
     might need (new APIs, locations, gotchas); build/test results.
2. **Answer questions.** If the agent returns a question, answer it from the task sheet or ask the user,
   then continue the same agent with `SendMessage` (it keeps its context).
3. **Verify.** Read the diff yourself. Run `cargo build` for the affected crates (the whole workspace if
   shared crates changed), `cargo fmt`, and the done-criteria checks. On problems, send them back to the
   same agent and re-verify.
4. **Commit the task alone.**
   - Stage only this task's files by path (never `git add -A` / `git add .`); leave unrelated changes
     such as `todos.md` edits or stray files unstaged.
   - Message in the repo style: `feat: ...`, `fix: ...`, `refactor: ...`, lowercase, one line, plus a
     short body only if useful.
   - **No AI attribution** of any kind (no `Co-Authored-By` for an assistant, no "Generated with" line);
     this overrides default attribution behavior. Author email must be in
     `.github/allowed-commit-emails.txt`.
5. **Update carry-forward notes** from the agent's report: add what later tasks need, drop what no
   remaining task needs.
6. **Blocked task:** if a task cannot be finished, stop and ask the user (skip, retry with changes,
   or abort). Never silently skip a task or merge two tasks into one commit.

## Phase 3: Report

List each task with its commit hash and one-line summary, plus anything skipped, deviated from, or left
for follow-up.

## Common mistakes

| Mistake | Fix |
|---|---|
| Starting task 1 while task 3 still has open questions | All questions for all tasks come first |
| Generic questions asked without reading the code | Read crate docs and code, then ask concrete questions |
| Brief dumps every earlier report into the next agent | Only the carry-forward notes this task needs |
| Brief omits a fact the previous task established (new API, moved file) | Check carry-forward notes before every brief |
| Agent picks a design option on its own | Brief tells it to stop and return the question |
| Committing the agent's work unverified | Read the diff, build, fmt, then commit |
| `git add -A` sweeps in unrelated changes | Stage task files by path |
| Several tasks in one commit | One task, one commit |
