# Feature: {{short name}}

<!--
File name: .opencode/docs/features/NNN-<kebab-name>.md (NNN = next unused number;
000 is project setup and is never reused).
Written by the plan agent after the product and engineering interviews are
complete. Executed by the build agent, which follows it top to bottom and does
not make design decisions of its own. Every section is mandatory; write "None"
rather than leaving one out.

Fill every {{...}} slot from this project's own context: code locations from
architecture.md §4, commands from architecture.md §2, design rules from
architecture.md §5. No "etc.", no "similar to above", no "TBD".
-->

**Status:** draft | approved | building | done
**Author:** plan agent, approved by luis
**Date:** {{YYYY-MM-DD}}
**Architecture version:** commit {{sha}} of `.opencode/docs/architecture.md`

---

## 1. Product requirements (PRD)

### 1.1 Problem
One paragraph. Who has the problem, when it occurs, what it costs them today.

### 1.2 Users and roles
Which roles touch this feature and what each may do. Cover the outsider and
the unauthenticated visitor explicitly, even when the answer is "nothing".
(This project has a single role — the developer at the terminal — and no
permission model; state that, then say what this feature lets them do.)

### 1.3 User stories
- As a {{role}}, I want {{capability}} so that {{benefit}}.
- ...

### 1.4 Acceptance criteria
Numbered, testable statements. Each one maps to at least one item in §4.
1. Given ..., when ..., then ...
2. ...

### 1.5 Out of scope
Explicit list of adjacent things this feature does **not** do.

### 1.6 Open questions resolved
Question the plan agent asked, and the answer the user gave. This is the audit
trail for every judgment call. Assumptions that were never asked go in the same
table with `A{{n}}` numbers and the label "assumption (not asked)".

| # | Question | Answer |
| --- | --- | --- |
| Q1 | | |

---

## 2. Engineering requirements (ERD)

### 2.1 Design considered
"Design it twice": at least two shapes were considered. State both in two or
three lines each and say why one was chosen. Reference the rule in
`architecture.md §5` that drove the decision.

**Option A:** ...
**Option B:** ...
**Chosen:** A, because ...

### 2.2 Domain changes
For every new or modified domain concept — type, entity, model, or module that
owns state or an invariant — the full declaration and its invariants. (This
project's domain types live in `src/conflict.rs` and `src/app.rs`; use the
field-by-field format below.)

```
{{Name}} ({{new | modified}})
  {{field}} : {{type}}{{, constraints}}
  {{validation, uniqueness, or state machine}}
```

Public methods, each with a full signature and a one-line contract:

```
{{Name}}.{{method}}({{args}}) -> {{type}}
  {{what it returns, when it returns nothing, what it raises}}
```

### 2.3 Data and migrations
One block per migration or schema file, in order, with the exact file name and
the exact columns. "None" with the reason when the feature adds no schema.
(This project has no database; the conflicted file in place is the only
persistence — say "None" and name the file format touched, if any.)

### 2.4 Services, jobs and side effects
Only when `architecture.md §4` says a service or a background unit is
warranted. Same signature-plus-contract format as §2.2. Every entry states its
idempotency rule and what happens when it is retried. (This project is
single-threaded with no background work; the only side effects are the file
write in `src/main.rs` and the git calls in `src/git.rs` — document them here
when a feature changes them.)

```
{{path}}
  {{Unit}}#{{method}}({{args}})
    {{what it does, how it is retried safely, what it returns when there is
      nothing to do}}
```

### 2.5 Routes
Exact route entries to add and where, by file path and by URL. Include the
entry files that ship with the route — its loading, error and not-found
states — because they are part of the change. For this project the equivalent
surface is the CLI in `src/main.rs` or the TUI screen in `src/ui/mod.rs`:
document the exact command form or screen state added.

```
{{path/to/routes.ext}}   {{METHOD}} {{/url}}
  {{what it loads, what it authorizes, success response, failure response}}
```

### 2.6 Authorization and ownership
The check applied to each route, and the exact response an unauthorized or
non-owner request receives. Name the rule from `architecture.md §8` that
demands it. (This project has none — OS file permissions only; write that
explicitly and name any new file path a feature touches.)

### 2.7 HTTP boundary
Per action: which relation or query it loads, what it authorizes, its success
response and its failure response. For a server-rendered framework this is the
controller; for a route-handler framework, the same information as handler
input, output and status codes. For this project: per action, its input (key
event or argv), the state it touches, its success result and its failure
result (message and exit code).

```
{{Unit}}   {{METHOD}} {{/url}}
  {{load}} -> {{authorize}} -> {{success: ...}} / {{failure: ...}}
```

### 2.8 Views and components
Every file, the component primitives it uses, and its rendering strategy. The
strategy must match `architecture.md §6`, and any file that breaks that rule
says why here. Include the empty state and the error state.

```
{{path/to/view.ext}}
  {{structure, primitives used, and the states it renders}}
```

### 2.9 Client-side behavior
Every unit of client behavior: the file, what runs on the client and why it
cannot run on the server, its inputs and its events. "None" is the common
answer, and it is a good answer. (This project has no client/server split —
the usual answer here is "None".)

### 2.10 External dependencies
For every external service or API this feature touches: the exact contract
relied on, the failure modes, retry and idempotency, timeouts and rate limits,
and the module that owns the call. Logic whose home depends on another
system's behavior is the most expensive thing to get wrong twice — write the
contract down, not the intent. (This project's only external system is the
`git` CLI; its full contract is in `architecture.md §2` and
`features/000-project-setup.md` §2.9. Re-state here only what the feature
changes.)

```
{{dependency}}
  contract: {{call, auth, request and response shape}}
  failures: {{each failure mode and what it becomes for the user}}
  owned by: {{module path}}
```

### 2.11 Configuration
Environment variables, framework config and framework flags. Each entry: the
name, its default, and what breaks when it is absent. Also any cache
invalidation or background schedule this feature introduces.

---

## 3. Test data and fixtures
Exact data to add, per file. Keep it minimal and named for its scenario
(`{{scenario_name}}`, not `one`). This project's fixtures are inline `&str`
constants in the `#[cfg(test)]` modules of `src/`; save-flow tests write
`std::env::temp_dir()/udiff-test-<pid>-<counter>` files.

---

## 4. Tests
Every test file and every test name. The builder writes exactly these tests, in
this order, before or alongside the code they cover. Each acceptance criterion
from §1.4 appears at least once in brackets. If a criterion is proven by a
command instead of a test, say so and name the command.

```
{{path/to/test.ext}}
  {{test name}}   [AC1]
  {{test name}}   [AC2]
```

---

## 5. Build order
Numbered steps the builder executes one at a time, verifying after each. A
step is one commit-sized unit, and a step's verify command is written on the
step — never a single verification at the end.

1. {{change}} -> {{verify command}}
2. {{change}} -> {{verify command}}
3. {{all verify commands from architecture.md §2}} -> all green
4. Update `architecture.md` if any name, table or rule changed

---

## 6. Verification checklist
The builder ticks these in the doc before reporting done.

- [ ] Every file in §2 exists at the exact path written there
- [ ] Every test in §4 exists with the exact name and passes
- [ ] `cargo clippy --all-targets -- -D warnings` reports no errors
- [ ] `cargo check --all-targets` exits zero
- [ ] Every acceptance criterion in §1.4 is referenced by a passing test or the
      command named for it
- [ ] `grep -rn "std::process\|Command::new" src/ | grep -v 'src/git.rs'`
      returns nothing (architecture.md §5 rule 4)
- [ ] `cargo tree --depth 1` lists no dependency besides `ratatui`
      (architecture.md §5 rule 1)
- [ ] `architecture.md` updated if any name, table or rule changed

---

## 7. Deviations
Filled by the builder. **What** was done differently from the plan: the section
and line of the plan, what it said, what was built instead. One row per
deviation. Write "None — executed verbatim" if empty.

| # | Plan section | Plan said | Built instead |
| --- | --- | --- | --- |
| D1 | | | |

---

## 8. Drifts
Filled by the builder, one entry per deviation in §7. **Why** the plan and
reality diverged, so the next plan does not repeat it. Each drift has a cause:

- `plan-gap` — the plan did not cover a case the code had to handle.
- `stale-context` — the plan described code or schema that had already changed.
- `convention-conflict` — the plan contradicted a rule in `architecture.md` or
  an existing pattern in the nearest similar file.
- `infeasible` — the plan could not pass as written (failing constraint,
  missing dependency, library behavior).
- `user-decision` — the user changed the requirement during the build.

```
D1
  Cause: {{one of the five}}
  Evidence: {{file, line, and the failure or conflict that exposed it}}
  Why the plan missed it: {{the specific assumption that was wrong}}
  Fix applied: {{what was changed}}
  Follow-up: {{none | update architecture.md §N | re-plan as NNN}}
```

Set `Status: done` only after §7 and §8 are filled and every §6 box is ticked.
