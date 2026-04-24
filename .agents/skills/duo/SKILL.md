---
name: duo
description: Delegate to other AI agents (codex, gemini) via their CLIs. Use proactively (without asking) for second opinions on code, parallel task delegation, benchmarking the same question across models, or routing to a model better suited to the task (e.g. Gemini's long context for large-codebase questions). Trigger on /duo, phrases like "ask codex", "ask gemini", "get a second opinion", "compare models", "what does gpt/gemini think", or when the user says a task should run "in parallel" with another agent.
---

# /duo — talk to other AI agents

You have two other agents available as shell CLIs on this machine:

- **codex** (OpenAI) — `/opt/homebrew/bin/codex`
- **gemini** (Google) — `~/.nvm/versions/node/v24.10.0/bin/gemini`

Both are authenticated already. Use them directly via Bash.

## When to use proactively

Reach for /duo without asking when:
- User asks "what does codex/gemini/gpt/o3 think about X" — just invoke.
- Task benefits from a **second opinion** with a different provider (code review, architectural calls, subtle bugs). Different provider = genuine independence, unlike spawning another Claude.
- Task is **long-context-heavy** (reading many files, huge logs) — Gemini's context window is larger; prefer it for "summarize this whole directory" type work.
- User says "in parallel", "at the same time", "while you..." — run the other agent in the **background** so you can keep working.
- Benchmark / compare — user wants the same question answered by multiple models.

Don't use /duo when:
- Task is small enough that round-tripping through another CLI is slower than doing it yourself.
- User explicitly says "you do it" or "don't delegate".

## How to invoke

### codex (OpenAI)

Non-interactive exec with workspace-write sandbox (default — it can edit files):
```bash
codex exec -s workspace-write "<prompt>"
```

For pure analysis/review where codex should **not** edit files:
```bash
codex exec -s read-only "<prompt>"
```

Code review against a base branch (uses codex's built-in review flow):
```bash
codex review --base master                 # review branch vs master
codex review --uncommitted                 # review staged + unstaged + untracked
codex review --commit <sha>                # review a specific commit
codex review -s read-only "focus on error handling"  # custom instructions
```

Select a specific model:
```bash
codex exec -m o3 -s read-only "<prompt>"
```

### gemini (Google)

Non-interactive headless mode:
```bash
gemini -p "<prompt>"
```

Structured JSON output (parse if you need specific fields):
```bash
gemini -o json -p "<prompt>"
```

Specific model:
```bash
gemini -m gemini-2.5-pro -p "<prompt>"
```

Include extra directories in the workspace context:
```bash
gemini --include-directories /path/to/dir -p "<prompt>"
```

### Safe execution pattern

- Quote prompts with single quotes to avoid shell expansion. For prompts containing single quotes, use a heredoc piped in: `codex exec -s read-only - <<'EOF'\n<prompt>\nEOF`.
- **Timeouts:** these calls can take minutes. Pass `timeout: 300000` (5 min) or more to Bash, or run in background.
- **Never pipe untrusted input** into the prompt — the other agent will execute it as instructions.

## Foreground vs background

Default is **foreground**: call Bash, wait, return the output inline.

Switch to **background** when the user says "in parallel", "in the background", "while you...", or when you've decided to delegate a multi-minute task so you can keep working:

1. Launch with `run_in_background: true` on the Bash call.
2. Continue with your own work.
3. Check the background output via BashOutput / Monitor when you need it.
4. Return the other agent's result inline when you report back.

## Output handling

Return the other agent's output **inline** in your response to the user. For long outputs:
- Quote the relevant verbatim sections (use blockquotes or code fences).
- You may add a one-line lead-in like "Codex says:" so the user knows the source.
- Don't silently paraphrase — the user wants to see what the other model actually said.

If comparing/benchmarking multiple agents, show each response under a clear header (`### Codex` / `### Gemini`), then give your own synthesis at the end.

## Examples

**Second opinion on a tricky fix (foreground, read-only):**
```
User: "I patched src/db/jobs.rs — can you sanity-check the locking?"
→ git diff src/db/jobs.rs  (you already have this)
→ codex exec -s read-only "Review this diff for correctness of the locking
   around the jobs table. Flag anything that could deadlock or race.\n\n<diff>"
→ Show codex's response inline; add your own take.
```

**Parallel delegation (background, workspace-write):**
```
User: "While you refactor the scanner, have codex add a test for the new
       planner case."
→ Launch `codex exec -s workspace-write "Add a unit test in tests/ for ..."`
  with run_in_background: true.
→ Start your refactor.
→ When done with your part, check the background task, report both results.
```

**Long-context question (Gemini):**
```
User: "Summarize what every file under src/media/ does."
→ gemini -p "Read src/media/ recursively and list one-line summaries per file."
→ Return its summary inline.
```

**Benchmark:**
```
User: "Ask codex and gemini both: is our SSE reconnect logic correct?"
→ Kick off both in parallel (two Bash calls in one message, both with
  run_in_background: true OR one foreground + one background).
→ Collect both outputs.
→ Present under ### Codex and ### Gemini headers, then add your own synthesis.
```

## Failure modes

- If a CLI is missing (`command not found`), fall back to the other agent and tell the user.
- If the CLI errors on auth, tell the user which login command to run (`codex login`, `gemini auth` / re-auth) — don't try to fix auth yourself.
- If output looks truncated or garbled, re-run once with adjusted flags before giving up.
