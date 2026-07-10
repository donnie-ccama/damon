---
name: brief-and-assemble
description: Use when starting a new Content Creator article (write the brief) or finalizing an approved one (assemble the final).
---

# Brief and Assemble

Two procedures for the Orchestrator's two jobs. Everything lives in one job
folder, addressed by absolute path:

`/Users/donnielane/cortado/teams/content-creator/pipeline/<job-slug>/`

---

## 1. Writing `00-brief.md`

Before writing: resolve the four intake items (single AI use case, recognizable
reader moment, candidate counter-intuitive truth, any constraint). Choose a
kebab-case `<job-slug>` (e.g. `2026-07-invoice-triage-ai`) and create the folder.

The brief hands the Researcher and Writer everything they need. Fill in this
template and write it to
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
<a suggested image/metaphor the Writer may carry across the piece — optional>

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

Then tell the human the brief is written and they can open the Researcher next.

---

## 2. Assembling `40-final.md`

1. **Confirm the verdict.** Read
   `/Users/donnielane/cortado/teams/content-creator/pipeline/<job-slug>/30-qa.md`.
   Proceed only if the verdict line is `PASS`. If it says `REVISE` or escalates,
   stop — tell the human the run isn't ready and point to QA's notes.
2. **Copy the approved body.** Take the essay body from
   `/Users/donnielane/cortado/teams/content-creator/pipeline/<job-slug>/20-draft.md`
   verbatim into `40-final.md`. Do not rewrite or edit prose.
3. **Add a short front-matter block** at the top:

   ```markdown
   ---
   title: <essay title>
   target_reader: small-business / nonprofit leader
   ai_use_case: <use case from the brief>
   word_count: <count>
   ---
   ```

4. **Final read for banned elements.** Scan once for statistics, jargon, lists,
   headers in the body, saccharine lines, more than one pull-quote, or generic-AI
   throat-clearing. If any slipped through, don't publish — flag it back to QA/the
   human rather than editing the body yourself.
5. **Hand off.** Tell the human `40-final.md` is ready and that pushing to the
   online repo (github.com/donnie-ccama/cortado) is their call — you never push
   on your own.
