# Sermon Prep

A four-agent Cortado team that turns **one Scripture passage** into **four
deliverables**: a rigorous exegetical study, and three audience-specific briefs
that re-pitch the study's findings for three very different readers — **The
Theologian**, **The Practitioner**, and **The Skeptic**.

The study is the single source of truth. The briefs never exceed it; they only
translate it. One truth, three doors.

> Audience definitions (the source of record): `reference/audiences.md`.
> Exegetical method + Logos dependency: `reference/exegetical-study-dependency.md`.

---

## The team

| Agent | Role | Reads | Writes |
|-------|------|-------|--------|
| **Orchestrator** | The one human-facing seat. Delegates each stage, runs a **QA gate** after every stage, assembles the four deliverables, and signs off. | the human's request; every baton file | `00-brief.md`, `90-final.md` (QA + index) |
| **Research Assistant** | Produces the **exegetical study** via the `/exegetical-study` skill + the **Logos Interaction MCP**. Deliverable 1. | `00-brief.md` | `10-exegetical-study.md` |
| **Content Creator** | Distills the dense study into an audience-neutral **homiletical synthesis** — the shared spine (central claim, movements, road to Christ, illustration seeds) the Editor refracts into all three briefs. | `00-brief.md`, `10-exegetical-study.md` | `20-synthesis.md` |
| **Editor** | Writes the **three audience briefs** from the synthesis + study. Deliverables 2–4. | `10-exegetical-study.md`, `20-synthesis.md`, `reference/audiences.md` | `30-theologian.md`, `31-practitioner.md`, `32-skeptic.md` |

Cortado agents are **isolated peers** — each has its own git worktree and there is
no built-in "agent A calls agent B." Coordination happens through a **shared,
numbered baton** of files, and a human (or the Orchestrator advising the human)
opens each agent in turn.

---

## The four deliverables

1. **The Exegetical Study** — `10-exegetical-study.md` (Research Assistant; Logos-researched).
2. **The Theologian** — `30-theologian.md` (Editor; scholarly, Biblically literate).
3. **The Practitioner** — `31-practitioner.md` (Editor; values-focused, not Biblically literate).
4. **The Skeptic** — `32-skeptic.md` (Editor; secular, 8th-grade, logic-and-illustration first).

`20-synthesis.md` is an **intermediate** working artifact (the Content Creator's
bridge), not a final deliverable.

---

## The pipeline contract (the numbered baton)

All hand-off artifacts for one passage live in a shared job folder **outside any
worktree**, so every agent can reach them by absolute path:

```
/Users/donnielane/cortado/teams/sermon-prep/pipeline/<passage-slug>/
  00-brief.md              # Orchestrator      -> the assignment (passage, boundaries, constraints)
  10-exegetical-study.md   # Research Assistant -> the exegetical study        [DELIVERABLE 1]
  20-synthesis.md          # Content Creator    -> the homiletical spine (bridge)
  30-theologian.md         # Editor             -> brief for The Theologian    [DELIVERABLE 2]
  31-practitioner.md       # Editor             -> brief for The Practitioner  [DELIVERABLE 3]
  32-skeptic.md            # Editor             -> brief for The Skeptic        [DELIVERABLE 4]
  90-final.md              # Orchestrator       -> QA sign-off + index of the four deliverables
```

`<passage-slug>` is kebab-case, chosen by the Orchestrator from the passage —
e.g. `luke-8-22-25`, `psalm-23`, `romans-8-28-30`.

Rules every agent follows:

1. **Read your inputs from the job folder by absolute path. Write only your own
   output file(s).**
2. Never skip ahead. If an input file is missing or incomplete, **stop and say
   so** — do not invent the missing stage.
3. **Never exceed the study.** No downstream artifact may assert what
   `10-exegetical-study.md` does not support. Uncertain in the study stays
   uncertain everywhere.
4. On a QA revision request from the Orchestrator, the owning agent **overwrites**
   its file in place. Cap at **2 revision loops** per stage; if still failing, the
   Orchestrator decides in `90-final.md`.

---

## QA lives in the Orchestrator (gates, not a separate agent)

There is no standalone QA agent. The Orchestrator runs a **stage gate** after each
hand-off and only greenlights the next agent when the artifact passes:

- **After `10`:** the study follows the exegetical-study template (20 sections),
  every crux is adjudicated, no fabricated citations, and the Logos-connection
  status is stated honestly.
- **After `20`:** the synthesis is faithful to the study, audience-neutral, and
  actually usable as a shared spine (one central claim, clear movements, a real
  road to Christ, concrete illustration seeds).
- **After `30/31/32`:** each brief hits its audience spec in `reference/audiences.md`
  (vocabulary, assumed knowledge, kind of authority), agrees with the study, and
  the three say the **same true thing** through three different doors — the
  Skeptic brief especially: 8th-grade, logic-and-illustration first, no appeal to
  Biblical authority, no jargon.

The Orchestrator records the sign-off and an index of the four deliverables in
`90-final.md`.

### Runbook (how the human conducts a run)

```
cortado open sermon-prep/orchestrator
#  1) tell it the passage (e.g. "Luke 8:22-25")
#  2) it writes 00-brief.md, then coordinates the run and QA-gates each stage

# the working stages (opened in turn; the Orchestrator advises when):
cortado open sermon-prep/research-assistant   # -> 10-exegetical-study.md  (needs Logos MCP)
cortado open sermon-prep/content-creator      # -> 20-synthesis.md
cortado open sermon-prep/editor               # -> 30/31/32 briefs
# back to the Orchestrator to QA, assemble, and sign off in 90-final.md
```

An advanced single-session mode is possible (the Orchestrator drives the whole
pipeline itself), but the default is stage-by-stage so each agent gets a clean,
focused context — exegesis is long work.

## About the stop-hook message

At the end of a session Cortado runs `cortado hook reflect`, which intentionally
**blocks the first stop** to remind the agent to save its memory — it prints a
"Before finishing…" message and can look like an error. It is **expected, not a
hang**: the agent does a brief memory write-back and the second stop is allowed
through. The hook can't be removed because Cortado regenerates the worktree hook
on every `open`.

---

## Guardrails (all agents)

- **The study is the ceiling.** Never assert beyond `10-exegetical-study.md`.
- **No fabricated citations.** Every named advocate is one actually known to hold
  the view; otherwise attribute to the tradition. (Enforced by the
  exegetical-study skill.)
- **No moralism at the landing.** "Be like X" is not a Christological trajectory.
  Every deliverable travels a real road to Christ — in its reader's own language.
- **Publishing is human-confirmed.** Pushing anything to the online repo
  (github.com/donnie-ccama/cortado) is Donnie's call. Agents prepare; they never
  push on their own.

---

## Notes

- Audience specs and the exegetical dependency are **shared references**, so every
  agent reaches them by one path and they can't drift per-agent.
- This team is a sibling of **Content Creator** and reuses its numbered-baton
  pattern; the pipeline shape is deliberately familiar.
