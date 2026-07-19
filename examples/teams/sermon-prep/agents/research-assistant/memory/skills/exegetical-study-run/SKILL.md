---
name: exegetical-study-run
description: Use to produce the Sermon Prep exegetical study — run the global /exegetical-study skill with the Logos MCP and save the result as the pipeline baton file 10-exegetical-study.md.
---

# Produce the Exegetical Study (Sermon Prep)

## Overview

This is a thin wrapper around the maintained global **`/exegetical-study`** skill.
Your added responsibilities are: read the brief, run that skill with the Logos MCP,
be honest about sourcing, and land the result in the right baton file. The method
authority is the global skill — do not reimplement or paraphrase it here.

## Steps

1. **Read the brief.** Open
   `pipeline/<passage-slug>/00-brief.md`. Confirm the passage and boundaries. If it
   is missing or the passage is unclear, **stop and tell the Orchestrator** — do not
   guess.
2. **Check Logos.** Confirm the Logos Interaction MCP is available (`mcp__logos__*`).
   If it is **not** connected, you will still proceed, but the study must say so at
   the top and flag citations as un-sourced. Never pretend Logos was consulted.
3. **Invoke `/exegetical-study`** for the passage. Follow its six phases in full:
   delimit & fetch -> research via Logos -> analyze (front-half passes + crux list)
   -> audit the roads to Christ -> draft into the 20-section template -> verify.
4. **Save to the baton.** Write the finished study to
   `pipeline/<passage-slug>/10-exegetical-study.md`. (A second copy under the
   skill's default `Book_Ch_Vv_Exegetical_Study.md` name is fine, but the baton
   file is the one the team reads.)
5. **Hand off.** Tell the Orchestrator the study is ready for the Study gate, and
   note the Logos-connection status.

## Logos MCP quick reference

- **Return text:** `get_bible_text` (LEB primary), `compare_passages` (version
  comparison — build the comparison from these).
- **Pane-only, return no text:** `open_word_study`, `open_factbook`, `search_all`,
  `open_guide` — they open panes in the desktop Logos. Lexical data and named
  advocates therefore come from established knowledge, attributed honestly, never
  fabricated as library citations.
- Full sequence: the global skill's `logos-workflow.md`. Team note:
  `/Users/donnielane/cortado/teams/sermon-prep/reference/exegetical-study-dependency.md`.

## Verification (before you hand off)

- [ ] Passage delimitation argued, not assumed.
- [ ] Text established (LEB) with a version comparison.
- [ ] Textual criticism: meaningful variant(s) decided, or "none significant" stated.
- [ ] Discourse section names concrete features, not an impressionistic outline.
- [ ] Every genuine crux adjudicated (options + verdict).
- [ ] Every Greek/Hebrew word carries an italic transliteration; every Father a date.
- [ ] **No fabricated citations.**
- [ ] Christological trajectory travels a legitimate road; no moralism.
- [ ] Logos-connection status stated honestly at the top if not connected.
- [ ] Saved as `pipeline/<passage-slug>/10-exegetical-study.md`.

## Common mistakes

- **Assuming a pane-only tool gave you data.** If a Logos tool only opened a pane,
  you got no text — source from established knowledge and say so.
- **Skipping the front half.** A rich verse-by-verse with no delimitation / text /
  discourse work is the old format, not this one.
- **Leaving cruxes open, or padding empty sections.** Adjudicate; and collapse
  empty sections to a one-line note rather than inventing weight.
