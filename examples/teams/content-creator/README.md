# Content Creator

A four-agent Cortado team that produces **technical AI use-case articles** written as
**Streetlight literary essays** — for small-business and nonprofit leaders who run traditional
software (Excel, CRM, QuickBooks) but are intimidated by AI. The team demystifies one AI use
case at a time, in Donnie Lane's voice, and never sounds like a tech blog.

> Voice source: `reference/streetlight-persona.md` (the "Streetlight Essayist" persona, applied
> **verbatim** — every rule, including *no statistics, studies, jargon, or listicles*).

---

## The team

| Agent | Role | Reads | Writes |
|-------|------|-------|--------|
| **Orchestrator** | Single-session **producer** — runs the whole article in one session: brief → research → **approval gate** → draft → self-edit → final | the human's request; web pages (accuracy checks only) | `00-brief.md`, `10-research.md`, `20-draft.md`, `40-final.md` |
| **Researcher** | Truth-and-texture scout (not a citation engine) — optional standalone stage | `00-brief.md` | `10-research.md` |
| **Writer** | Drafts the essay, persona rules verbatim — optional standalone stage | `00-brief.md`, `10-research.md` | `20-draft.md` |
| **QA** | **Opt-in** independent Streetlight quality-gate review, run after the fact; PASS or revision notes | `00`, `10`, `20` | `30-qa.md` |

Cortado agents are **isolated peers** — each has its own git worktree and there is no built-in
"agent A calls agent B." Coordination therefore happens through a **shared, numbered baton** of
files, and a human (or the Orchestrator advising the human) opens each agent in turn.

---

## The pipeline contract (the numbered baton)

All hand-off artifacts for one article live in a shared job folder **outside any worktree**, so
every agent can reach them by absolute path:

```
/Users/donnielane/cortado/teams/content-creator/pipeline/<job-slug>/
  00-brief.md      # Orchestrator → the assignment
  10-research.md   # Researcher   → the dossier
  20-draft.md      # Writer       → the essay draft
  30-qa.md         # QA           → PASS or REVISE + notes
  40-final.md      # Orchestrator → the approved, publish-ready essay
```

Rules every agent follows:

1. **Read your inputs from the job folder by absolute path. Write only your one output file.**
2. Never skip ahead. If your input file is missing or incomplete, stop and say so — do not invent
   the missing stage.
3. `<job-slug>` is kebab-case, chosen by the Orchestrator (e.g. `2026-07-invoice-triage-ai`).
4. On a REVISE loop, the Writer overwrites `20-draft.md` and QA overwrites `30-qa.md`. Cap at
   **2 revision loops**; if still failing, QA escalates to the Orchestrator in `30-qa.md`.

### Runbook (how the human conducts a run)

```
cortado open content-creator/orchestrator
#  1) tell it the one AI use case
#  2) it writes the brief + research, then PAUSES for your approval   <-- the only stop
#  3) approve -> it drafts, self-edits, and writes 40-final.md in one continuous pass
#
# optional, only when you want a formal independent review of the final:
cortado open content-creator/qa       # writes 30-qa.md: PASS or REVISE + notes
```

The **Researcher** and **Writer** agents still exist and can be opened to run
those stages by hand if you'd rather split the work up.

## About the stop-hook message

At the end of a session Cortado runs `cortado hook reflect`, which intentionally
**blocks the first stop** to remind the agent to save its memory — it prints a
"Before finishing…" message and can look like an error. It is **expected, not a
hang**: the agent does a brief memory write-back and the second stop is allowed
through. The single-session flow above keeps this to about **once per run**. The
hook can't be removed because Cortado regenerates the worktree hook on every
`open`.

---

## Shared audience & purpose (all agents)

- **Reader:** owns or leads a small business or nonprofit, often alone at the top. Fluent in
  business operations and traditional software (Excel, CRM, QuickBooks). **Not** fluent in
  software development or AI/agentic terminology, and intimidated by "cutting-edge AI."
- **Job of every article:** take **one** concrete AI use case in the everyday workplace and make
  a tired, skeptical reader feel *seen and steadied* — curious enough to try it, never lectured.
- **Demystify by parallel:** explain each AI concept through a familiar business-software analogy,
  in slow, patiently crafted illustration. Curiosity over completeness.

---

## The Streetlight quality gate (QA's checklist)

An essay PASSES only if **all** of these hold:

**Form**
- [ ] 1000–1200 words (hard limit).
- [ ] 7–10 paragraphs. No headers, no lists.
- [ ] Second person throughout ("you"); occasional inclusive "we."
- [ ] Exactly **one** sustained metaphor, carried across the whole piece.
- [ ] Exactly **one** pull-quote, landed in the middle third.
- [ ] Long comma/em-dash sentences balanced by short declaratives; rhythm varies deliberately.
- [ ] Italics used for pivotal phrases / voiced inner thoughts.

**Move**
- [ ] Opens on a recognizable moment the reader instantly knows.
- [ ] Names the common story, then overturns it with one counter-intuitive truth.
- [ ] Names the **counterfeit** of the virtue (the failure mode on the other side).
- [ ] Gets embarrassingly concrete about what it looks like hour to hour.
- [ ] The skeptic's gritty objection is named out loud **and honestly answered** (no greeting-card).
- [ ] Closes on a low-stakes invitation; final line circles back to the opening image.

**Truth**
- [ ] The AI capability described is real and accurately characterized (matches `10-research.md`).
- [ ] Faith-informed stance present but invisible — grace, calling, dignity of unseen faithfulness
      — with **zero** religious vocabulary, scripture, or cited authority.

**Bans (any hit = REVISE)**
- [ ] No statistics, studies, or cited numbers.
- [ ] No jargon or unexplained AI/dev terminology.
- [ ] No listicles or bulleted structure in the essay body.
- [ ] No saccharine lines; no more than one pull-quote.
- [ ] No generic-AI throat-clearing ("In today's rapidly evolving landscape…", "game-changer",
      "here's why this matters" as a bridge).

QA writes `30-qa.md` as: a one-line **verdict** (`PASS` or `REVISE`), then, if REVISE, a short
numbered list of specific, actionable fixes tied to the checklist items above.

---

## Notes

- The persona is embedded in the Writer's and QA's skills so it travels with the team and every
  agent can reach it without an external file path.
- This team is deliberately cloneable: a future **encouragement / Biblical-perspective** team can
  reuse the same pipeline with a different brief and a relaxed research remit.
