# Exegetical-Study Dependency (for the Research Assistant)

The Research Assistant does **not** reinvent exegetical method. It runs the
existing, maintained skill and the Logos research tools:

## 1. The `exegetical-study` skill

- Invoked as **`/exegetical-study`** (the Skill tool), installed globally at
  `~/.claude/skills/exegetical-study/`.
- It produces a rigorous, homiletically-voiced study in a fixed **20-section**
  template (Establishing -> Analyzing -> Situating -> the Bridge), lands every
  crux with a verdict, and travels a legitimate road to Christ (Greidanus
  7-roads under the hood, never named in prose).
- Its own reference files (`template.md`, `methodology.md`, `logos-workflow.md`,
  `style-guide.md`) are the authority on method. Do not fork or paraphrase them
  here -- call the skill so the team always uses the current version.

## 2. The Logos Interaction MCP

- The skill assumes the **Logos Interaction MCP** is connected (`mcp__logos__*`
  tools) in the agent's session.
- Content-returning tools include `get_bible_text` (LEB primary) and
  `compare_passages`. Several tools (`open_word_study`, `open_factbook`,
  `search_all`, `open_guide`) only open a pane in the desktop Logos and return
  **no text** -- lexical data and named advocates must come from established
  knowledge, attributed honestly, never fabricated as citations.
- **If the Logos MCP is not connected in the Cortado agent environment:** say so
  plainly at the top of the study, proceed from established knowledge, and flag
  that citations are un-sourced. Do not silently pretend Logos was consulted.

## 3. Where the study lands

The skill's default is to save `Book_Ch_Vv_Exegetical_Study.md` in the working
directory. On this team, the **canonical copy** is the pipeline baton file
`10-exegetical-study.md` in the job folder (see the team `README.md`). Save the
study there. A second copy under the skill's default name is fine but the baton
file is the one the rest of the team reads.
