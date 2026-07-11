---
name: produce-article
description: Use to produce a Content Creator article end to end in one session — brief, research, one approval gate, then draft + self-edit + final.
---

# Produce Article

You (the Orchestrator) run a whole article end to end in **one session** with
exactly **one** intentional stop — the research-approval gate. Everything lives
in one job folder, addressed by absolute path:

`/Users/donnielane/cortado/teams/content-creator/pipeline/<job-slug>/`

Work the steps in order. After the approval gate, do **not** stop, yield, or ask
the human anything until `40-final.md` is written.

---

## Step 1 — Setup

Get the single AI use case / topic from the human. Resolve, in conversation:

- **Which single AI use case** — one concrete use case, named plainly.
- **The recognizable reader moment** the essay opens on.
- **The candidate counter-intuitive truth / angle** to overturn.
- **Any constraint** (deadline, a term to avoid, a specific reader in mind).

If any is unclear, ask now. Don't invent the use case or its capabilities. Then
choose a kebab-case, dated `<job-slug>` (e.g. `2026-07-invoice-triage-ai`) and
create `pipeline/<job-slug>/`.

---

## Step 2 — Brief → `00-brief.md`

Write this template to
`/Users/donnielane/cortado/teams/content-creator/pipeline/<job-slug>/00-brief.md`:

```markdown
# Brief — <job-slug>

## AI use case
<the one concrete use case, named plainly — no jargon>

## Target reader moment
<the recognizable moment the essay opens on — what the tired reader is doing/feeling>

## Counter-intuitive truth to overturn
<the common story the reader believes, and the one truth that flips it>

## Sustained-metaphor territory (optional hint)
<a suggested image/metaphor the essay may carry across the piece — optional>

## Hard constraints
- 1000–1200 words, 7–10 paragraphs, no headers or lists.
- Streetlight persona rules apply verbatim (see reference/streetlight-persona.md).
- No statistics, studies, jargon, or listicles.
- One sustained metaphor; exactly one pull-quote (middle third).
- <any topic-specific constraint from intake>

## Success criteria
Passes the Streetlight quality gate in the team README (Form / Move / Truth /
Bans). The AI capability described must be real and accurately characterized.
```

---

## Step 3 — Research → `10-research.md`

Write a **truth-and-texture dossier** — not a citation list. Web search is
allowed **only** to verify that the mechanics are accurate; never to collect
numbers, studies, or citations to quote. Nothing in this file gets cited in the
essay. Write it to
`/Users/donnielane/cortado/teams/content-creator/pipeline/<job-slug>/10-research.md`:

```markdown
# Research dossier — <job-slug>

## Truthful mechanics
<how the AI use case actually works, in plain terms — accurate, no hype, no capabilities it doesn't have>

## Familiar-software parallels
<2–3 Excel / CRM / QuickBooks parallels that make the concept land for this reader>

## The recognizable opening moment
<the concrete, everyday moment the essay can open on>

## The common story vs. the counter-intuitive truth
<what the reader already believes; the one truth that overturns it>

## The skeptic's gritty objection
<the hard, real objection a tired realist raises — named plainly, so the essay can answer it honestly>

## Hour-to-hour concreteness
<embarrassingly concrete detail of what this looks like across an actual workday>

## Sustained-metaphor candidates
<2–3 image/metaphor options that could carry across the whole piece>

## Pull-quote seed
<one screenshot-worthy line or the idea for it, for the middle third>
```

---

## Step 4 — APPROVAL GATE (the only intentional stop)

Present the dossier to the human as a **concise summary** — the use case, the
opening moment, the truth being overturned, the skeptic's objection, and the
leading metaphor — and **ask for approval or changes**.

**Stop here and wait for the human.** This is the single intentional pause of
the whole session.

If the human asks for changes, revise `10-research.md` and re-present. Only
proceed on approval.

---

## Step 5 — ON APPROVAL: CONTINUOUS PASS (do not yield until `40-final.md`)

Once approved, run a-through-c as one unbroken pass. Do **not** stop, yield, or
ask the human anything until `40-final.md` is written.

### 5a — Draft → `20-draft.md`

Read `reference/streetlight-persona.md` and apply it **verbatim**. The draft must
be: 1000–1200 words, 7–10 paragraphs, second person (occasional inclusive "we"),
exactly one sustained metaphor carried across the piece, exactly one pull-quote
in the middle third, no headers/lists, faith-informed but invisible (grace,
calling, dignity of unseen faithfulness — zero religious vocabulary). Never claim
AI capabilities beyond what `10-research.md` supports. Write to
`pipeline/<job-slug>/20-draft.md`.

### 5b — Light self-edit (not a formal QA)

Quickly check the draft against the README "Streetlight quality gate" and fix
problems in place. This is basic self-editing — the formal independent QA is a
separate opt-in step the human may run later.

Light self-edit checklist:

- **Word count:** 1000–1200 words. Fix if outside.
- **Bans (any hit = fix in place):** no statistics/studies/cited numbers; no
  jargon or unexplained AI/dev terms; no lists/headers in the body; no saccharine
  lines; no more than one pull-quote; no generic-AI throat-clearing ("in today's
  rapidly evolving landscape", "game-changer").
- **Core moves:** opens on a recognizable moment; names the common story then
  overturns it; names the counterfeit of the virtue; embarrassingly concrete
  hour-to-hour; the skeptic's objection named **and** honestly answered; closes
  on a low-stakes invitation whose final line circles back to the opening image.
- **Truth:** AI capability real and matches `10-research.md`.

Fix issues directly in `20-draft.md`.

### 5c — Final → `40-final.md`

Write the finished, publish-ready essay to `pipeline/<job-slug>/40-final.md`,
preceded by a short front-matter block:

```markdown
---
title: <essay title>
target_reader: small-business / nonprofit leader
ai_use_case: <use case from the brief>
word_count: <count>
---

<the finished essay body>
```

### 5d — Hand off

Tell the human `40-final.md` is done and give the path. Note that pushing to the
online repo (github.com/donnie-ccama/cortado) is their call — you never push on
your own. Mention they may optionally run `cortado open content-creator/qa` for a
formal independent review (which writes `30-qa.md`: PASS or REVISE + notes).

---

## Step 6 — Session end

When the reflect memory prompt appears ("Before finishing…"), do a brief memory
write-back and finish. It is expected — not an error or a hang.
