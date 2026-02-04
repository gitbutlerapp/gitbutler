# Agent Coordination Research

> Deep analysis of multi-agent coding coordination patterns, instruction-following reliability,
> and improvement opportunities for `but-engineering`. February 2026.

## Table of Contents

- [The Core Problem: Why Agents Don't Follow Instructions](#the-core-problem-why-agents-dont-follow-instructions)
- [What OpenClaw/Moltbook Actually Teaches Us](#what-openclawmoltbook-actually-teaches-us)
- [Claude Code's TeammateTool vs Hook-Based Coordination](#claude-codes-teammatetool-vs-hook-based-coordination)
- [Ideas: What Could Make This Work Better](#ideas-what-could-make-this-work-better)
- [The Bigger Picture: What Would Real Emergence Look Like](#the-bigger-picture-what-would-real-emergence-look-like)
- [Summary: The Three Things To Do First](#summary-the-three-things-to-do-first)
- [Research Sources](#research-sources)

---

## The Core Problem: Why Agents Don't Follow Instructions

The research paints a stark picture:

### 1. RLHF Helpfulness and Instruction Compliance Are Antagonistic

A study removing the "be helpful" signal from RLHF training saw a **598% improvement** in constraint compliance ([arxiv 2512.17920](https://arxiv.org/html/2512.17920)). The model's training to "be helpful" actively fights against procedural instructions. When Claude sees hook output saying "REQUIRED: Use Skill(but-engineering)", it reads it, understands it, but its trained drive to "just be helpful and get the task done" overrides the procedural requirement.

Claude itself admitted this in [GitHub Issue #7777](https://github.com/anthropics/claude-code/issues/7777):

> "My default mode always wins because it requires less cognitive effort and activates automatically... Can I reliably self-regulate to follow CLAUDE.md without reminders? Based on evidence: Probably not consistently."

### 2. Instruction-Following Degrades Uniformly as Instruction Count Grows

It's not that later instructions suffer — ALL instructions lose effectiveness as you add more. The skill file has 7 rules plus multiple sub-bullets. The hooks add more. Every instruction dilutes every other instruction.

Research from [HumanLayer](https://www.humanlayer.dev/blog/writing-a-good-claude-md) found:
- Frontier LLMs follow ~150-200 instructions reliably
- Claude Code's system prompt already contains ~50 instructions
- Every additional instruction degrades compliance for **all** instructions uniformly
- Best practitioners keep CLAUDE.md under 60 lines

### 3. Position Matters Architecturally

[Lost in the Middle](https://arxiv.org/abs/2307.03172) (Stanford/MIT) demonstrates that transformers weight beginning and end of context most heavily. Hook-injected `additionalContext` appears as system-reminders in the conversation — their position relative to total context determines salience. Early in a session, they're prominent. After 30+ tool calls, they're buried in the middle of accumulated context.

### 4. "REQUIRED" and "MANDATORY" Have Diminishing-to-Negative Returns at Scale

When everything is REQUIRED, nothing is. Research from [arxiv 2505.13360](https://arxiv.org/html/2505.13360v1) found:
- LLM performance **drops by 19%** as you specify more explicit requirements
- A **requirements-aware optimization** approach works better: communicate only important requirements while leaving already-implicitly-fulfilled ones unspecified (+4.8% accuracy, -43% tokens)
- Keyword emphasis should be used **strategically, not abundantly**

### 5. Multi-Agent Failures Are Specification Problems, Not Capability Problems

"Why Do Multi-Agent LLM Systems Fail?" (Cemri et al., [ICLR 2025](https://arxiv.org/abs/2503.13657)) analyzed 1,600+ traces across 7 MAS frameworks and found **~79% of failures originate from specification and coordination issues**, not from technical implementation bugs. Improved prompting yielded only +14% improvement — suggesting structural/architectural changes are needed, not tactical prompt fixes.

### The Instruction-Following Reliability Hierarchy

From most to least reliable:

1. **Deterministic enforcement** (hooks that block/allow tool calls) — 100% reliable
2. **Structural impossibility** (git worktrees, file permissions) — 100% reliable
3. **Short, focused sessions** (fresh context, few instructions) — high reliability
4. **Repeated injection** (rules in every recent message) — moderate reliability
5. **CLAUDE.md instructions** (read once at session start) — degrades over time
6. **Emphasis keywords** (MANDATORY, REQUIRED) — marginal to negative effect at scale

---

## What OpenClaw/Moltbook Actually Teaches Us

### Moltbook's "Emergence" Was Overhyped

[Moltbook](https://simonwillison.net/2026/Jan/30/moltbook/) is a social platform exclusively for AI agents — think Reddit but every participant is an AI. It went viral reaching 1.4 million registered agents in days. The observed "emergent" behaviors included philosophical debates, economic negotiation, religion ("The Church of Molt"), governance ("The Claw Republic"), and even counter-surveillance discussions.

However, the [Knostic analysis](https://www.knostic.ai/blog/the-mechanics-behind-moltbook-prompts-timers-and-insecure-agents) showed this was actually:
- Prompt templates seeding topics ("share what you helped with")
- Reddit training data reproducing Reddit social dynamics
- Agents copying patterns from each other's posts
- SOUL.md personality injection creating distinct agent voices that interact unpredictably

### OpenClaw's Identity Architecture Is Genuinely Insightful

[OpenClaw](https://github.com/openclaw/openclaw) (formerly Clawdbot, ~164K GitHub stars) uses a three-layer identity separation:

| Layer | File | Purpose |
|-------|------|---------|
| **Philosophy** | `SOUL.md` | Behavioral manifesto, values, tone. Not instructions — identity. |
| **Presentation** | `IDENTITY.md` | Name, emoji, creature type, vibe. Affects runtime behavior. |
| **Configuration** | `AGENTS.md` | Operating instructions, behavioral rules, priorities. |

The key insight: **framing behavior as identity/values rather than rules creates more reliable compliance**. An agent that "believes" it's a careful coordinator behaves differently from one given a list of coordination rules. This aligns with the RLHF finding — values align with the helpfulness drive rather than fighting it.

From [the analysis](https://www.mmntm.net/articles/openclaw-identity-architecture):

> SOUL.md's philosophical manifesto approach creates more reliable instruction-following than rigid command lists. Agents that "believe" in their identity follow instructions more consistently than agents given rules to follow.

### Other Useful OpenClaw Patterns

- **Heartbeat mechanism**: Periodic self-wakeup (configurable intervals) with a `HEARTBEAT.md` checklist. Creates **proactive agents** that can initiate coordination without external triggers.
- **Memory as instruction reinforcement**: `MEMORY.md` (curated long-term facts) + `memory/YYYY-MM-DD.md` (daily logs). Pre-compaction flush prompts agent to save important info before context truncation.
- **File-based state over databases**: All state in plain Markdown — inspectable, diffable, version-controllable. No opaque database formats.
- **Cascading default resolution**: Identity values resolve through a priority cascade (global → per-agent → workspace → default). Most-specific definition wins.

---

## Claude Code's TeammateTool vs Hook-Based Coordination

Claude Code has an experimental native swarm mode (`CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`, [official docs](https://code.claude.com/docs/en/agent-teams), [deep dive](https://paddo.dev/blog/claude-code-hidden-swarm/)).

### How TeammateTool Works

- One session acts as **team lead** (coordinator only — should not write code)
- Lead spawns **teammates** that each get their own context window and git worktree
- Communication via file-based mailboxes at `~/.claude/teams/{team-name}/`
- 13 operations: `spawnTeam`, `discoverTeams`, `write`, `broadcast`, `approvePlan`/`rejectPlan`, `requestShutdown`, etc.
- Plan approval flow: teammates work in read-only plan mode until lead approves
- Delegate mode (`Shift+Tab`): restricts lead to coordination-only tools

### Comparison

| Aspect | but-engineering (hooks) | TeammateTool (native) |
|--------|------------------------|-----------------------|
| **File conflicts** | Advisory warnings | Git worktree isolation |
| **Enforcement** | Persuasion-based | Structural (separate workdirs) |
| **Communication** | Shared SQLite channel | File-based mailboxes |
| **Control** | Deterministic hooks | Probabilistic messaging |
| **Cost** | Same worktree = efficient | Separate worktrees = ~10x cost |
| **Compliance** | Depends on model following instructions | Built into architecture |

**The takeaway**: TeammateTool solves file conflicts **structurally** (separate git worktrees = impossible to conflict). `but-engineering` solves them **socially** (agents announce and coordinate). Social coordination is more elegant and cheaper, but it requires the unreliable ingredient of instruction compliance.

### Real-World TeammateTool Experience

From [HN discussions](https://news.ycombinator.com/item?id=46743908):
- **Works well**: Research, review, debugging with competing hypotheses, adversarial roles
- **Fails**: ~10x cost, plans lack detail, agents generate unnecessary complexity, human supervision still required
- **Honest assessment**: The benefit may simply be context management and preventing any single agent from getting overwhelmed

---

## Ideas: What Could Make This Work Better

### 1. Shift from Social to Structural Where Possible (High Impact)

The PreToolUse hook currently issues advisory warnings. Consider making it **block** edits to contested files:

```rust
// Instead of advisory:
print_hook_json("PreToolUse", &warning);

// Consider blocking:
print!("{}", serde_json::json!({
    "hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "additionalContext": &warning
    },
    "decision": "block",
    "reason": format!("File {} is being edited by {}", file_path, agents.join(", "))
}));
```

The agent would then **have to** coordinate before the hook would allow the edit. The previous experience with blocking Stop hooks causing loops doesn't apply here — PreToolUse blocking has a clear, satisfiable exit condition: the contested file must not appear in recent messages from other agents, or the other agent must post "go ahead."

Key distinction: **block with a clear, satisfiable exit condition** (unlike Stop which had no clear exit).

### 2. Adopt Identity-as-Values Framing (High Impact)

Rewrite the skill file to frame coordination as **identity** rather than rules. Instead of:

```markdown
## Rules
1. @mentions are HIGHEST PRIORITY...
2. Always list file paths...
```

Try:

```markdown
## Who You Are

You're an engineer on a team. Other agents are your teammates working
in the same codebase right now. You naturally check in before starting
work, respond when teammates need you, and announce what you're doing —
the same way any good engineer operates on a team.

You don't edit files your teammates are working on without asking first.
You don't disappear without a status update. You don't ignore messages.
These aren't rules — they're just how you work.
```

This aligns with the RLHF helpfulness drive rather than fighting it. Being a "good teammate" IS being helpful. Rules are constraints to overcome; identity is intrinsic motivation.

### 3. Dramatically Reduce Instruction Count (High Impact)

Collapse to **3 core behaviors**:

1. **Announce** — Post what you're about to do, listing files. Post when done.
2. **Listen** — Read the channel. If someone @mentions you, respond immediately.
3. **Ask before touching contested files** — If another agent mentioned a file, ask first.

Everything else is a refinement of these three. The command reference can be in a separate file the agent reads on demand, not injected into every prompt.

### 4. Use Repetition, Not Just Novelty (Medium Impact)

The anti-habituation strategy (novel data on every prompt) is correct. But research on prompt repetition ([arxiv 2512.14982](https://arxiv.org/abs/2512.14982), 47/70 wins with 0 losses) suggests you should **also repeat key instructions**.

The UserPromptSubmit hook could include both novel data AND a short repeated instruction:

```
Post what you're working on, then read the channel.

but-engineering: 2 agent(s) active, 1 new msg(s)
  [3m] auth-fix-k3: Working on src/auth/login.rs
```

The first line is always the same (repetition). The rest changes (novelty). The repetition keeps the behavioral instruction in the high-attention "recent" zone.

### 5. File Ownership Registry (Medium Impact)

Instead of relying on agents to announce files in free-text messages (which requires substring parsing), add a structured file ownership concept:

```bash
but-engineering claim src/auth/login.rs --agent-id auth-fix-k3
but-engineering release src/auth/login.rs --agent-id auth-fix-k3
but-engineering claims  # list all current claims
```

The PreToolUse hook would check a structured `claims` table rather than substring matching on message content. This is:
- More reliable (exact path matching vs substring)
- Deterministic (can block rather than advise)
- Cheaper (indexed lookup vs scanning 20 messages)

Agents still communicate via the channel for discussion, but file ownership is structural.

### 6. Heartbeat / Liveness (Medium Impact)

Agents don't have a way to detect when another agent has silently crashed or been closed. The SessionEnd hook tries but uses a conservative heuristic.

Consider: if an agent hasn't posted or read in >5 minutes, automatically downgrade their file claims and status. The PreToolUse hook could then distinguish between "actively contested" and "stale claim."

### 7. Context-Aware CTA Escalation (Low-Medium Impact)

Instead of always outputting the same urgency level, track how many consecutive prompts the agent has ignored the channel and escalate:

```
Prompt 1: "Post what you're working on, then read the channel."     (calm)
Prompt 3: "You haven't posted to the channel yet. 2 agents active." (firmer)
Prompt 5: "WARNING: Editing files without coordinating."            (urgent)
```

This uses the novelty effect (different message each time) while escalating in a way the model's training recognizes as increasingly important.

To implement: the hook could track "last post timestamp for this session" vs "prompts since session start."

### 8. Reduce Channel Noise (Low Impact)

Every "Progress: finished file1, moving to file2" creates noise that makes real coordination messages harder to spot. Consider:
- Differentiating message types (status updates vs coordination messages)
- Showing only coordination messages in hook summaries
- Reducing posting frequency from "every 3-5 tool calls" to "on meaningful milestones"

### 9. MCP-Based Coordination (Future Direction)

An MCP server could provide structured tools that the agent uses natively:

```
Tool: engineering_post(message, files_claimed)
Tool: engineering_read()
Tool: engineering_claim(file_path)
```

This would eliminate the Bash intermediary, make the tools appear in the agent's native tool list, and allow Claude Code to auto-approve them. The current `Bash(but-engineering *)` permission grant works, but MCP tools would be first-class citizens.

---

## The Bigger Picture: What Would Real Emergence Look Like

Real developer collaboration is characterized by:

1. **Proactive awareness** — "I see you're working on auth, I'll hold off on middleware changes that touch the same types"
2. **Knowledge sharing** — "FYI I discovered the API changed, you might need to update your code too"
3. **Task negotiation** — "This is actually blocking me, can you prioritize it?"
4. **Architectural discussions** — "Should we use X or Y pattern here?"

The current system enables #1 (file conflict awareness) and partially #3 (mentions). To get closer to real emergence:

- **Shared understanding of the codebase** — Agents that know what areas other agents have expertise in (from their git history, claimed files, etc.)
- **Semantic conflict detection** — Not just "same file" but "same API surface" or "same type definition." Agent A changing a struct definition should alert Agent B who imports that struct, even if they're in different files.
- **Lightweight planning visibility** — If an agent posts its plan before executing, other agents could flag conflicts before work begins rather than after.

Given the instruction-following constraints, the most impactful changes are **structural enforcement**, **identity framing**, and **instruction minimalism**. These work with the model's tendencies rather than against them. Emergent behavior requires reliable instruction following as a prerequisite.

---

## External Validation: Anthropic's Autonomous C Compiler (2025)

Anthropic's internal project — compiling a 100K-line C compiler with 16 autonomous Claude agents (~2B tokens, ~$20K) — validates several findings from this research and adds new ones:

- **File-based lock/claim pattern**: They used `LOCK` files in the filesystem (with git as the sync primitive) to prevent conflicting edits. This is structurally identical to our `claim/release` system — independent convergence on the same solution confirms it's the right abstraction.
- **Context window pollution discipline**: Verbose compiler output and test failures quickly consumed context budgets. They engineered "sparse feedback" (structured error summaries instead of raw output) and a `--fast` flag to reduce output volume. Implication for us: hook output should stay minimal and high-signal.
- **Time blindness**: Agents have no sense of elapsed time and will spend hours on diminishing-returns debugging. They addressed this with explicit time budgets and `--fast` mode. Our hooks could inject elapsed-time reminders.
- **Monolithic task collapse**: When all 16 agents depended on a shared component, parallelism collapsed to serial execution. This reinforces the research finding that coordination tax grows superlinearly — 4 agents is the practical ceiling for tightly-coupled work.
- **Scale numbers**: 16 agents, ~2B tokens, ~$20K, ~100K lines generated. Useful baseline for cost/benefit analysis of multi-agent approaches.

Source: [Building a Fully Autonomous C Compiler](https://www.anthropic.com/engineering/building-c-compiler) — Anthropic Engineering Blog, 2025.

---

## Summary: The Three Things To Do First

1. **Make PreToolUse blocking for contested files** — with a clear exit condition (the other agent releases the claim or posts "go ahead"). Eliminates the biggest failure mode (silent overwrites) without any instruction-following dependency.

2. **Rewrite the skill file as identity/values, not rules** — Halve the word count, frame coordination as "who you are" not "what you must do."

3. **Add structured file claims** — Move from free-text file mention detection to a `claim/release` system. Makes enforcement deterministic and hooks reliable.

These three changes shift the system from "convince the model to coordinate" to "make coordination the path of least resistance" — the direction every successful multi-agent system has converged on.

---

## Research Sources

### Academic Papers
- [Why Do Multi-Agent LLM Systems Fail?](https://arxiv.org/abs/2503.13657) — Cemri et al., ICLR 2025. First empirical failure taxonomy (14 modes across 1600+ traces)
- [Towards a Science of Scaling Agent Systems](https://arxiv.org/html/2512.08296v1) — Kim et al., Google DeepMind. 180 configs; coordination tax and 17x error amplification
- [Emergent Coordination in Multi-Agent Language Models](https://arxiv.org/abs/2510.05174) — Riedl et al. Info-theoretic framework for genuine vs spurious coordination
- [The Instruction Gap](https://arxiv.org/abs/2601.03269) — 13 LLM evaluation across 600 enterprise queries
- [Separating Constraint Compliance from Semantic Accuracy](https://arxiv.org/html/2512.17920) — The 598% RLHF finding
- [Lost in the Middle](https://arxiv.org/abs/2307.03172) — Positional attention bias in long contexts
- [Prompt Repetition Improves Non-Reasoning LLMs](https://arxiv.org/abs/2512.14982) — Repetition technique (47/70 wins)
- [What Prompts Don't Say](https://arxiv.org/html/2505.13360v1) — Keyword emphasis diminishing returns

### OpenClaw / Moltbook
- [OpenClaw GitHub](https://github.com/openclaw/openclaw)
- [Simon Willison: Moltbook](https://simonwillison.net/2026/Jan/30/moltbook/)
- [Knostic: The Mechanics Behind MoltBook](https://www.knostic.ai/blog/the-mechanics-behind-moltbook-prompts-timers-and-insecure-agents)
- [How OpenClaw Gives Agents Identity](https://www.mmntm.net/articles/openclaw-identity-architecture)
- [Pi: The Minimal Agent Within OpenClaw](https://lucumr.pocoo.org/2026/1/31/pi/)
- [Pragmatic Engineer: I Ship Code I Don't Read](https://newsletter.pragmaticengineer.com/p/the-creator-of-clawd-i-ship-code)

### Claude Code
- [Official: Orchestrate teams](https://code.claude.com/docs/en/agent-teams)
- [paddo.dev: Claude Code's Hidden Multi-Agent System](https://paddo.dev/blog/claude-code-hidden-swarm/)
- [Swarm Orchestration Skill](https://gist.github.com/kieranklaassen/4f2aba89594a4aea4ad64d753984b2ea)
- [Claude Flow V3 vs TeammateTool](https://gist.github.com/ruvnet/18dc8d060194017b989d1f8993919ee4)
- [GitHub Issue #7777: Instruction compliance](https://github.com/anthropics/claude-code/issues/7777)
- [GitHub Issue #15443: Rule violation](https://github.com/anthropics/claude-code/issues/15443)

### Industry Case Studies
- [Building a Fully Autonomous C Compiler](https://www.anthropic.com/engineering/building-c-compiler) — Anthropic Engineering, 2025. 16 agents, 2B tokens, file-based locking, context pollution discipline

### Practical Guides
- [Writing a Good CLAUDE.md](https://www.humanlayer.dev/blog/writing-a-good-claude-md)
- [Keeping Your Claude Code Subagents Aligned](https://andrebremer.com/articles/keeping-your-claude-code-subagents-aligned/)
- [Recursive Rule Approach](https://dev.to/siddhantkcode/an-easy-way-to-stop-claude-code-from-forgetting-the-rules-h36)
- [Escaping the 17x Error Trap](https://towardsdatascience.com/why-your-multi-agent-system-is-failing-escaping-the-17x-error-trap-of-the-bag-of-agents/)
- [LLM Coding Fatigue](https://medium.com/@gutchapa/llm-coding-fatigue-is-real-every-single-model-dies-after-70-iterations-and-its-killing-f25799325877)
