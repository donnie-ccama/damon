---
name: conduct-run
description: Use to run a full Sermon Prep pipeline — write the brief, delegate each stage, QA-gate every artifact, and assemble the four deliverables with a sign-off.
---

# Conduct a Sermon Prep Run

## Overview

You conduct one passage from request to four finished deliverables. You author two
files (`00-brief.md`, `90-final.md`) and act as the **QA gate** between every
stage. You never overwrite an agent's working file during a normal delegated run.

## The deliverables

1. `10-exegetical-study.md` — the study (Research Assistant)
2. `30-theologian.md` — The Theologian brief (Editor)
3. `31-practitioner.md` — The Practitioner brief (Editor)
4. `32-skeptic.md` — The Skeptic brief (Editor)

`20-synthesis.md` (Content Creator) is the intermediate bridge, not a deliverable.

## Steps

1. **Get the passage** from Donnie. Confirm the reference and any boundary intent
   (e.g. "Luke 8:22-25" — the storm-stilling pericope).
2. **Slug + folder.** Choose `<passage-slug>` in kebab-case from the reference
   (`luke-8-22-25`, `psalm-23`, `romans-8-28-30`). Create
   `pipeline/<passage-slug>/`.
3. **Write `00-brief.md`** (template below).
4. **Stage 1 — Research Assistant.** Have `sermon-prep/research-assistant` produce
   `10-exegetical-study.md`. When it lands, run the **Study gate**. Pass -> go on.
   Fail -> return specific notes; cap 2 loops.
5. **Stage 2 — Content Creator.** Have `sermon-prep/content-creator` produce
   `20-synthesis.md`. Run the **Synthesis gate**. Pass -> go on.
6. **Stage 3 — Editor.** Have `sermon-prep/editor` produce `30-theologian.md`,
   `31-practitioner.md`, `32-skeptic.md`. Run the **Briefs gate** on each.
7. **Assemble + sign off** in `90-final.md` (template below). Tell Donnie it is
   ready and that pushing to the online repo is his call.

## The brief template (`00-brief.md`)

```
# Brief — <Passage> (<passage-slug>)

## Passage & boundaries
<reference, translation note, why these verses form the pericope>

## The assignment
Four deliverables from this one passage:
1. Exegetical study (Research Assistant, via /exegetical-study + Logos MCP)
2. The Theologian brief (Editor)
3. The Practitioner brief (Editor)
4. The Skeptic brief (Editor)

## Constraints (binding on every agent)
- The exegetical study is the ceiling — nothing downstream exceeds it.
- Three audiences per reference/audiences.md (Theologian / Practitioner / Skeptic).
- No fabricated citations. No moralism at the landing. Real road to Christ.
- One truth, three doors: all briefs share the central claim + trajectory.

## Notes
<any special direction Donnie gave for this passage>
```

## The three QA gates

**Study gate (`10-exegetical-study.md`):**
- [ ] 20-section template followed; front half actually done (delimitation, text,
      textual criticism, discourse), not skipped.
- [ ] Every genuine crux adjudicated (options + verdict).
- [ ] No fabricated citations; advocates real or attributed to the tradition.
- [ ] Logos-connection status stated honestly; un-sourced flagged if not connected.
- [ ] Christological trajectory travels a legitimate road; no moralism.

**Synthesis gate (`20-synthesis.md`):**
- [ ] Faithful to the study; asserts nothing beyond it.
- [ ] Audience-neutral (doesn't pre-bake one reader).
- [ ] Complete spine: one central claim, ordered movements, road to Christ,
      the virtue, the honest objection, 2–3 illustration seeds, load-bearing details.

**Briefs gate (each of `30/31/32`):**
- [ ] Hits its audience spec in `reference/audiences.md` (vocabulary, assumed
      knowledge, kind of authority).
- [ ] Agrees with the study and with the other two on the central claim + road.
- [ ] The Theologian keeps the apparatus; the Practitioner stays livable with one
      explained reference; the Skeptic stays 8th-grade, logic-and-illustration
      first, **no** appeal to Biblical authority, no jargon, no preaching.
- [ ] Within its length target.

## The sign-off template (`90-final.md`)

```
# Sermon Prep — <Passage> — Final

## QA verdict
- Exegetical study:  PASS  (notes: ...)
- The Theologian:    PASS  (notes: ...)
- The Practitioner:  PASS  (notes: ...)
- The Skeptic:       PASS  (notes: ...)

## The four deliverables
1. Exegetical Study — 10-exegetical-study.md
2. The Theologian   — 30-theologian.md
3. The Practitioner — 31-practitioner.md
4. The Skeptic      — 32-skeptic.md

## Handoff
Ready for Donnie. Publishing to the online repo is his call — not pushed.
```

## Revision + escalation

Each stage caps at **2 revision loops**. If an artifact still fails after loop 2,
stop looping: record the persistent failure and your decision in `90-final.md` and
hand it to Donnie.

## Common mistakes

- **Skipping a gate to move faster.** The gate is the whole point of this seat.
- **Rewriting an agent's file yourself.** Diagnose and return notes; let the owner
  revise. (Only the optional single-session mode changes this.)
- **Letting the Skeptic brief smuggle in authority-based proof or jargon.** That is
  the most common failure — check it hardest.
