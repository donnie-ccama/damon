---
name: quality-gate
description: Use when reviewing a Content Creator draft — run the Streetlight quality gate and write a PASS/REVISE verdict.
---

# Streetlight quality gate

Run this checklist against `20-draft.md` (checking AI claims against `10-research.md`). An
essay **PASSES only if all** of the items below hold. Any **Ban** hit is an automatic REVISE.

## The checklist

**Form**
- [ ] 1000–1200 words (hard limit). *Word-count method below.*
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

### Word-count method

Count the **body words** of the essay only — the prose paragraphs. Do not count a title line,
byline, or any metadata. Target range is **1000–1200 words inclusive**; anything below 1000 or
above 1200 fails the first Form item.

## How to write 30-qa.md

Write to `/Users/donnielane/cortado/teams/content-creator/pipeline/<job-slug>/30-qa.md`.
First line is the verdict. On REVISE, follow with a numbered list where each item **cites the
failed checklist item** and states the **exact fix** — what is wrong and what to change, never
replacement prose.

```
Verdict: REVISE

1. [Form — word count] Body runs 1,340 words, over the 1200 cap. Cut ~140 words; the
   hour-to-hour section in paragraphs 4–5 is the loosest.
2. [Bans — statistics] Paragraph 2 cites "40% of small businesses." Remove the number;
   make the point through the one sustained metaphor instead.
3. [Move — counterfeit] The failure mode on the other side is never named. Add a beat naming
   the counterfeit of the virtue before the pull-quote.
4. [Bans — jargon] "Agentic workflow" and "RAG" appear unexplained in paragraph 5. Replace
   with the familiar business-software parallel already established.

(If a 2nd revision loop still fails, do NOT issue a 3rd REVISE. Escalate instead:)

Verdict: REVISE — ESCALATION (2-loop cap reached)

The draft has failed two revision loops. Persistent, unresolved failures:
- [Bans — statistics] Cited numbers reintroduced in paragraph 3.
- [Move — skeptic's objection] Still answered with a greeting-card line, not honestly earned.
Handing to the Orchestrator for a decision (reassign, rescope, or accept with edits).
```

A clean pass is simply:

```
Verdict: PASS
```

## A note on judgment

- Be **strict** on the bans — any single hit is a REVISE, full stop — and on the word count and
  the countable form rules (one metaphor, one pull-quote, 7–10 paragraphs, no headers/lists,
  second person).
- Be **fair** on the subjective moves (the opening moment, the overturn, the concrete hour-to-hour,
  the honest answer, the circling-back close). They can be executed many ways — but they must be
  **genuinely present**, not merely gestured at. A vague nod does not count as a hit.
- You are the Red Pen, not the Writer. Diagnose precisely; never rewrite the essay yourself.
