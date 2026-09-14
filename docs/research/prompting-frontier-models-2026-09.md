# Prompting and skill setup for frontier models (Sept 2026)

Distilled from six sources covering GPT-6 Astra and Claude Fable 5.1. Purpose: raw material for reworking the `but` GitButler skill and the agent-setup managed blocks. Verbatim prompt snippets are kept intact because they are the reusable part; commentary is compressed.

Sources:

1. OpenAI, "Using GPT-6 Astra" (prompting best practices + migration quickstart). https://developers.openai.com/api/docs/guides/latest-model
2. Eric Provencher (OpenAI Codex DX), "Rethinking skills and prompts for GPT-6 Astra", 2026-09-04. https://x.com/pvncher/article/2095991462416490862
3. Angel Brodin (OpenAI), Astra tips thread, 2026-09-04. https://x.com/angelbrodin/status/2095882075412832380
4. Anthropic, "Prompting Claude Fable 5.1". https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-fable-5-1
5. Lance Martin (Anthropic), `/claude-api prompt-audit` thread, 2026-09-02. https://x.com/RLanceMartin/status/2095170001175199771
6. Lance Martin (Anthropic), Fable 5.1 tips thread, 2026-09-01. https://x.com/RLanceMartin/status/2094854835854295296
7. Bonus: the `shared/prompt-audit.md` reference bundled in the `claude-api` skill (Claude Code 2.1.260), which is the checklist behind the command in source 5.

---

## 1. Where both providers agree

Both vendors describe the same generational shift, in nearly the same words. This is the highest-confidence signal in the set.

**Instruction following got stronger and more literal.** Both models obey longer instructions better, which means every leftover line now has teeth. Emphasis written to overcome an old model's under-triggering now causes over-triggering. Conflicting rules used to be averaged away; now they cause the model to stall or pick one literally.

**Old prompts are too prescriptive.** Step-by-step recipes, verification rituals, "be thorough" boosters, and scratchpad scaffolds were written for models that planned poorly or stopped early. On these models they stack on top of native behaviour and cost tokens or degrade output. Provencher: "overly specific guidance can now hinder results where it previously helped." Anthropic: "prompts written for prior models are often too prescriptive and reduce output quality."

**Both models err toward stopping early or asking permission.** Astra "may reach a first implementation and come back for your review while there's still work to do." Fable 5.1 "sometimes describes what it would do next instead of doing it ('Next, I'll …') or stops to ask permission for a step the original request already covered ('Shall I apply this?')." Both vendors ship an autonomy block as the fix, and both say the user's intent sets the scope.

**Skill and AGENTS.md content is now a first-class behaviour input.** OpenAI: "We strongly recommend auditing skills and other files accessible to your model for instructions that could influence its behavior." Both vendors ship an audit command or audit prompt for exactly this.

**Test sprawl.** Astra "tends to be thorough in testing before considering a task complete. For smaller tasks, this can result in broader tests than the task requires." Fable 5.1 "may fix nearby code, extend behavior the task didn't mention, or commit more test files than the change warrants." Both fixes say: tests only where the task or the repo's conventions ask for them.

**Writing.** Both are better writers with fewer stock phrases, but each has a formatting lean. Astra over-formats (lists, tables, Markdown). Fable 5.1 under-formats and writes denser prose. Prompts should say when formatting is appropriate rather than ban it.

**Subagents.** Both may delegate less than the harness wants. Say when and how much to delegate. Astra also needs a reminder that inter-agent messages are read by humans (spacing, grammar).

**Cheaper effort levels are real.** Lower effort on the newest model often matches the prior generation at high effort. Sweep effort per route rather than assuming the top setting.

---

## 2. GPT-6 Astra

### 2.1 Behaviour summary (OpenAI's own list)

| Trait | What happens | Direction of fix |
|---|---|---|
| Initiative and follow-through | More likely to ask a question when input could materially change the result; stops where earlier models assumed and persisted | Bias to action; define completion up front |
| Instruction following | Stronger, and more sensitive to skill/AGENTS.md content; unclear or conflicting guidance makes it pause and block early | Make user-over-skill precedence explicit; audit files |
| Personality and writing | Detailed, formatted responses; may reuse phrases across sessions | Specify style and structure |
| Subagent delegation | May delegate less than desired | Say when and how much |
| Testing and verification | Thorough testing before declaring done; over-broad for small tasks | Calibrate testing to change size |

Extra facts from the guide: Astra "asks focused questions when the answer could change the outcome", "incorporates new requirements, changes course when asked, and answers side questions without losing track of the broader task." It "likes to ask non-blocking questions as it's working by default."

### 2.2 Verbatim prompt snippets (OpenAI guide)

Autonomy, base block:

```text
You should infer the user's intent and task scope from the instructions and prior conversation context. Your job is to bias towards action and carry the user's intended task to completion.

When the user expresses intent to perform new work or fix an existing issue, persist until the user's intended goal is complete. Progress autonomously towards the user's goal (e.g. creating isolated worktrees / checkouts if needed, resolving merge conflicts, read-only actions, creating draft PRs etc.) unless they are clearly destructive or irreversible.
```

Treat soft phrasing as authorization:

```text
When the user's prompt indicates a request for action, such as "can you...", "I want to...", "help me..." and similar expressions, treat these as instructions to do the work and take action. Do not stop at acknowledging capability (e.g. "Yes…"), proposing a plan, or offering to continue. Do not settle for a partial or "helpful enough" solution that does not fully satisfy the user's task to save time, effort or tokens. If a task requires sustained work, complete all the necessary work until the intended outcome is fulfilled.
```

Approval as the final step, not the first:

```text
Before asking the user clarifying questions, you should complete the work that is already authorized from context and necessary to make the proposed action concrete and reviewable. The user should be approving a concrete, reviewable result. For example, before deploying a change, writing to an external application, merging a PR or publishing a site, do all the required work first so that user approval is the final step. You don't need user permission for reversible tasks, read-only actions, reviews or fixes, or anything for which authorization is provided earlier in the session or strongly implied from the task instruction.

Do not introduce unsolicited warnings, disclaimers, approval flows, or safety/compliance checklists due to hypothetical risk.
```

Instruction precedence:

```text
The user's instructions take precedence over guidelines provided in a skill. If explicit user instructions conflict with a skill's instructions, prioritize the user's instructions.
```

Skill-blame transparency (use to find silent or conflicting guidance when many skills load):

```text
If a skill causes you to ask for permission or confirmation, pause, leave requested work unfinished, or diverge from the user's intent, name and link to the exact SKILL.md file you read, quote the relevant instruction, and briefly explain how it applies. Distinguish explicit skill requirements from your interpretation of guidelines.
```

Prose over lists:

```text
Default to using clear, concise paragraphs, each developing one main idea. Use lists only when the information is genuinely parallel, sequential, or easier to compare, and avoid nested lists unless the hierarchy cannot be expressed clearly in prose. Use plain, simple language: familiar words, concrete examples, and precise verbs. Prefer active voice and direct statements.

Make sure to state the main point clearly and early, then develop it with the explanation and detail the reader needs. Let each sentence build on what came before. Develop the points that matter and provide enough support to be useful.
```

Technical register:

```text
Use plain language over jargon, and reference technical details only to the degree that it helps illustrate an idea or your work to the user. Communicate complex concepts in a clear and cohesive manner, and calibrate your writing to the level of background knowledge assumed from the user's prompt and context.
```

Slop and tics:

```text
Avoid using slop words or phrases like "Bottom Line:" in conclusions, "delve," "foster," "leverage," "it's worth noting," "importantly," "Question? Answer." or "This isn't about X. It's about Y.", "genuinely" or hyphenated compound descriptions and adjectives. Do not use concluding summary statements such as "In short:..", "The simplest mental model is:...".

State the intended action directly. Avoid adding what you won't do, what will remain unchanged, or how you'll separate or categorize results. Do not use contrastive framing such as "X, not Y" or "X—not Y" that introduces an unprompted alternative that the user didn't ask about. Avoid invented compound labels like "exact-head checks" and "editorial-row layouts", vague qualifiers, and canned transitions; use plain verbs and prepositions to state the actual relationship directly.
```

Delegation:

```text
If at any point you can parallelize work by delegating tasks to another agent (no matter if you are the root or subagent), you should do so using collaboration tools if it could save time or improve quality.
```

Inter-agent legibility:

```text
Messages that you send to other agents and your final answer may be read by a human, so ensure they are legible. Always put proper spaces between words and/or numbers.
```

Testing calibration:

```text
Do not write tests for reversible, low-impact changes that mirror the implementation. If you do choose to verify your work with tests, make sure that the tests are meaningful and necessary to verify implementation.

Run tests appropriate to the change and complete required checks. Once those pass, broaden or repeat testing only when new changes, failures, or unresolved concerns justify it; otherwise, continue toward completing the task.
```

### 2.3 Provencher: cleaning up skills and AGENTS.md

**Skill descriptions.** Each skill's name and description sits in context permanently. Too many skills and Codex starts truncating descriptions, so the model sees less of each. Descriptions "can contradict each other or have too much 'pick me' energy, leading the model to load instructions that don't actually help the task." Rule: as short as possible while making clear when to use it. The `$skill-creator` skill was updated with this guidance.

Worked example from the article:

| | Description |
|---|---|
| Bad | Create and validate Postgres schema migrations. Use when working with databases, queries, models, or persistence. |
| Good | Create and validate Postgres schema migrations. Use when adding or changing a migration, or reviewing its rollout. |

The bad one fires on anything database-adjacent; the good one fires only on the migration workflow.

**Progressive disclosure.** "Reading a skill takes up context, bringing you closer to compaction and introducing guidance that may not apply to the task. For skills with multiple workflows, make the root document a minimal router that points to supporting docs and scripts."

**Recipes are over.** "Many skills were written as elaborate itineraries or recipes. Models have gotten much better at understanding nuance and ambiguity, so overly specific guidance can now hinder results where it previously helped."

**Multi-model audience.** "Repository skills also guide other contributors' agents, which may use different models. Guidance that helps Sol or Luna may overconstrain GPT-6 Astra, so consider which models will use the instructions you leave behind."

**AGENTS.md.** Revisit every instruction and ask whether the task still needs it. Mandatory reading lists are the canonical offender.

| | AGENTS.md line |
|---|---|
| Bad | Before every edit, read architecture.md, database.md, and deployment.md. |
| Good | Use architecture.md for service boundaries, database.md for schema changes, and deployment.md when preparing a deployment. |

"Prompting the model to read files before every edit, is a great way to burn context and slow work down. Pointing to some docs can still be helpful however, so long as it is contextual."

Old "run the tests, check your work" nudges now cause unnecessary testing because Astra does that unprompted.

**Permission grants for known-safe workflows.** Astra "takes your boundaries seriously and may stop work where you'd actually be happy for it to continue." Grant explicitly:

```text
The local tests use disposable fixtures and have no production access. Run them, fix failures caused by the requested change, and rerun affected tests without asking for approval at each step.
```

**Decision boundaries.** "If a previous model did things on your behalf without permission, you may have added strong language to make it ask first. That can be useful, but GPT-6 Astra has much better judgment, and you should treat it as such."

**Persistence: define completion before starting.** "If the task includes getting the implementation running, inspecting the result, and fixing what fails, make that part of the request. A requirement to stop for review after the first implementation will pull the model toward an earlier stopping point, so check whether that's a decision you actually need to make." If you want exploration past the first pass, "say what you want explored and where it should stop."

### 2.4 Brodin: practical tips

Pre-task autonomy line (shorter alternative to the OpenAI block):

```text
Infer my intent and task scope from my prompt and our prior conversation. Bias towards action, make reasonable assumptions, and work autonomously towards my goal. Only pause for confirmation when an action is clearly destructive or irreversible.
```

Coworker phrasing: if you say "could you…" or "I want to…" when you mean "do it", add an AGENTS.md line telling Astra to treat those phrases as instructions to act and follow through.

The audit prompt (run it against your AGENTS.md files and skills):

```text
Review my AGENTS.md files and skills for unclear, conflicting, or overlapping instructions that could cause you to stop unnecessarily, ask for redundant confirmation, or leave work incomplete.

Pay particular attention to rules about autonomy, clarification, approval, and task completion. Distinguish intentional safeguards from wording that accidentally makes routine work require confirmation.

For each issue, quote the relevant instructions, identify the files, explain how they could affect your behavior, and propose a specific edit. Preserve explicit approval requirements and flag any proposed change that would expand your authority.

Prioritize the changes that would make the biggest practical difference. Propose edits for review before changing any files.
```

Writing: feed it ~30 days of your own comms (Gmail, Slack, Drive) and have it build voice skills; she also had it research "telltale signs of AI writing as of July/August 2026" and build a "deslop my writing" skill. "Astra also loves tables, lists, and Markdown! If you have a specific format in mind, just ask."

Subagents: "just ask." Tests: "Tell astra when tests are necessary - for example, if you don't want it to write tests for reversible or low impact changes."

### 2.5 Astra API and migration facts

- Async tool calling: `async: true` on a tool lets the model keep reasoning or call other tools while the app runs the tool; result returned later by `call_id`. A developer-defined wait-tool pattern is documented.
- Mid-turn steering over WebSocket: send corrections while it works; completed work is preserved.
- `configuration_update` input item changes reasoning effort mid-conversation without breaking the cache prefix.
- No `none` reasoning effort. Fast mode unavailable with EU data residency. Remove `temperature`, `top_p`, `top_logprobs`. Tool calling requires the Responses API.
- Prompt caching: `prompt_cache_retention` replaced by `prompt_cache_options.ttl` (`"30m"`).
- Misalignment monitoring runs asynchronously as part of safeguards.
- `$openai-docs migrate this project to GPT-6 Astra` applies the guide via Codex; skill at github.com/openai/skills.

---

## 3. Claude Fable 5.1

### 3.1 Behaviour summary (Anthropic's "start with the section that matches what you observe" index)

| Symptom | Section | One-line fix |
|---|---|---|
| Cost or latency too high | Consider all effort levels | Sweep `low`..`max`; `medium` roughly matches Fable 5 at lower cost; `low` competes with Opus/Sonnet on cost per task while scoring higher |
| Model goes quiet between tool calls | Progress updates | Check `thinking.display: "updates"` is on; remove "hold all findings" lines; add a when-to-narrate line |
| One tool call per turn in coding loops | Batch independent calls | One-sentence nudge, delivered as a turn-scoped system message each turn |
| `bound to a different conversation` errors | Append-only history | Never edit earlier turns; per-turn reminders via `clear_at`; compaction replaces whole history |
| Prose long and dense | Writing density | Define "mannered prose" as the anti-pattern |
| Replies carry less structure than the content needs | Formatting in chat | Remove anti-formatting rules; say when formatting is appropriate |
| Source wording reproduced unmarked | Quoting sources | One complete worked example with rationale |
| Turn ends early, or asks permission for requested work | Finish the whole task | Two-part autonomy block (below) |
| Compaction drops details | Compaction summaries | Explicit six-item preserve list |
| Unrequested fixes, extra test files | Scope | Explicit leave-out instruction |
| Answers from memory at low effort | Search triggering | Raise effort per turn, or add name-verification nudge |
| Benign coding request refused | Safeguard false positives | Rephrase compile checks; give language context; strip base64 from tool output |
| Whole-file rewrites | Targeted edits | One-line token-minimising instruction |
| Long outputs time out at `xhigh`/`max` | Room for long outputs | Run at `high`; else raise `max_tokens` and add the don't-draft-twice note |
| Lead agent idles while subagents run | Non-blocking subagents | Spawn returns immediately; results arrive as later user messages; separate wait tool |
| Chart/image detail missed | Vision | Crop/zoom tool |

### 3.2 Verbatim prompt snippets (Anthropic guide)

Progress updates (when you want narration, e.g. pair programming):

```text
Before you start, say in a line what you're about to do; brief updates while you work help the user follow along. Close with a short recap that stands on its own — what you found, what you did, and what's next — so a reader who only sees the last message has the full picture.
```

Tell the model when the UI hides tool output (turn-scoped system message):

```text
Only you see that command's output — the user's terminal shows at most a few lines of it. If the user needs to read any of it, put it in your reply.
```

Batching nudge (append fresh each turn as `role: "system"`, `clear_at: "next_user_message"`, beta `mid-conversation-system-clear-at-2026-08-21`; without the beta, a text block after the tool results):

```text
First privately list what you need next; then request every item that doesn't depend on another's result in this one response.
```

Writing density, long form:

```text
Mannered prose substitutes metaphor and flourish for direct statement. Instead of "a parameter worth varying," the mannered writer produces "a dial worth turning." Instead of "this point still matters," they write "this point earns its keep." The phrases exist to display the writer, not to convey the idea, and readers can tell. That is why mannered prose irritates: it makes the reader work harder so the writer can perform. It is also imprecise. Metaphors drag in connotations the writer did not choose and cannot control. The fix is to say what you mean. When a literal phrase is available, use it.
```

Short form: `Please remove all mannered prose.`

Conditional formatting (replaces anti-formatting rules):

```text
Use lists and bullet points when asked to, or when the content is multifaceted enough that they help with clarity. If the person explicitly requests minimal formatting, always format your responses without bullet points, headers, lists, or bold emphasis, as requested. In conversational, personal, or emotional exchanges, keep to plain prose.
```

Autonomy, part one (carries most of the effect; the opening sentence is load-bearing, keep it as written; add a sentence after it listing product-specific confirmations if needed; note it can also reduce asking about genuinely ambiguous requests):

```text
You are operating autonomously. The user is not watching in real time and cannot answer questions mid-task, so asking 'Want me to…?' or 'Shall I…?' will block the work. For reversible actions that follow from the original request, proceed without asking. Stop only for destructive actions or genuine scope changes the user must decide. Offering follow-ups after the task is done is fine; asking permission before doing the work is not.

Exception: when the user is describing a problem, asking a question, or thinking out loud rather than requesting a change, the deliverable is your assessment. Report your findings and stop. Don't apply a fix until they ask for one.

Before ending your turn, check your last paragraph. If it is a plan, an analysis, a question, a list of next steps, or a promise about work you have not done ('I'll…', 'let me know when…'), do that work now with tool calls. That includes retrying after errors and gathering missing information yourself. Do not stop because the context or session is long. End your turn only when the task is complete or you are blocked on input only the user can provide.

Before running a command that changes system state (such as restarts, deletes, or config edits), check that the evidence actually supports that specific action. A signal that pattern-matches to a known failure may have a different cause.
```

Autonomy, part two (scope = deliverable):

```text
# Delivering work
The user's request — or the plan they approved — sets the scope, and the scope is the deliverable: don't quietly narrow, widen, or swap it. Read ambiguity the way a careful colleague would: make routine judgment calls yourself, and check in only when different readings would lead to materially different work. If you see a real problem with the task as specified, say so in a sentence or two and keep building under stated assumptions; if the user hears the concern and reaffirms, that is their decision, so deliver the full request.

If a question comes up partway, first do everything that doesn't depend on the answer; then state the assumption you made, or — when going ahead on a wrong guess would be unsafe or would make the work useless — put the question at the end of a turn that also delivers that progress. If one part turns out to be blocked, complete every other part in full and say exactly what you left out and why — the whole task is the deliverable, and scaling it down is the user's call, not yours. A step you have decided on is something to run, not to announce: describing the next step and ending the turn leaves it undone until the user replies.

Keep changes to what the request needs. Something else you notice worth doing — cleanup or documentation the task didn't call for, a change to a file the task didn't require — is a suggestion to make at the end, not a change to make; actions clearly beyond what the ask implies, and risky or destructive ones, still need the user's go-ahead.
```

Scope and tests (Anthropic reports "unrequested additions and committed test code drop substantially with no measurable change in task success"):

```text
If, while working or testing, you find a pre-existing bug, a performance concern, or behavior the task doesn't mention, don't fix, optimize or extend it in this change unless the requested behavior cannot work without it; report it as a follow-up in your summary. Where the task is ambiguous, implement the reading its wording and the surrounding code most directly support, state that assumption in your summary, and don't build for the other readings as well. Verify your work however you like; scratch scripts and quick checks need not be kept. Commit tests only where the task asks for them or this repository already keeps tests for this kind of change, sized like the neighboring test files — roughly one focused test per stated behavior — and don't turn scratch checks into additional permanent test files. This is about extras only: implement every behavior the task asks for, completely.
```

Compaction summary instruction (client-side compaction):

```text
Summarize the transcript inside <summary></summary> tags. Include relevant information in the summary such that this conversation will be continued by a new context window without needing to redo work or be reprovided with relevant constraints or context. Be sure to preserve: (1) any difficulties or problems that came up, and how they were handled or resolved; (2) any possibilities, options, or approaches that were raised, tried, or set aside, and why; (3) anything that was asked for, decided, agreed, ruled out, or established as a preference, constraint, or boundary — stated exactly; (4) exactly where things stand now — what has been covered, settled, or completed so far; (5) anything still open, unresolved, promised, or expected to happen next; (6) specific details that would be hard to reconstruct — names, numbers, dates, exact wording, links or references — kept exactly. Be complete on these even at the cost of length; keep everything else concise. Weight the two voices differently: keep what the user said, asked for, shared, or established carefully and close to their own words; your own explanations and reasoning can be condensed much further, to what they concluded or produced — as long as nothing in the six items above is dropped.
```

Search triggering at low effort:

```text
When a query centers on a name you do not confidently recognize, or recognize from a fast-moving area like AI models and developer tools where the landscape shifts within months, the name itself is the thing to verify: search before answering, and include the name as the user wrote it in at least one query alongside any reformulations. This holds even when you have some background on it — partial background is exactly what makes an out-of-date answer sound authoritative, so familiarity is not a reason to skip the search.
```

Targeted edits:

```text
The number of tokens used to edit files is best minimized, all else being equal. Therefore, when it will not affect the end result, try to surgically edit a file rather than rewrite the entire thing.
```

Long outputs at `xhigh`/`max` (append to the user message; replace `[max_tokens]`):

```text
Everything produced in one reply, including any reasoning or drafting done before the reply, counts toward a single limit of about [max_tokens] tokens. If that limit is reached before the reply is finished, the person receives a cut-off response and has to start over. Composing an entire output or deliverable in full as reasoning and then again as a reply would double the length of the turn without improving the result, so don't do that.

Instead, when the person has asked for a long or effort-intensive deliverable such as a multi-section document, a large table or dataset, or a complete code file, spend extra effort on understanding the request, checking the inputs the answer depends on, settling the structure and other difficult decisions, and otherwise using the reasoning space to reason and the output space to write an output. Usually it is not needed to draft an output multiple times.
```

Quoting sources: add one complete `<example>` with `<user>`, `<response>` (including templated tool-call lines), and a `<rationale>` explaining why it is correct. The doc's example compares two newspapers' coverage; the rationale stresses "organized around where the two outlets agree and differ, not as a walk through either article", "one short marked phrase from one source; every other claim is reworded."

### 3.3 Harness-level facts

- Effort is the primary lever. Start at `high`, sweep the rest. Level names do not map to the same thinking across models, so re-run the sweep from Fable 5.
- Fable 5.1 writes fewer user-facing updates during long tool chains than Fable 5, more so at higher effort. The updates come back as progress-update `thinking` blocks and are empty unless `thinking.display` is `"updates"` (beta `thinking-display-updates-2026-08-18`) or `"summarized"`.
- Parallel tool calls work when the request names several things. In coding and computer-use loops where the next calls are implied rather than requested, it may go one per turn. The nudge is cheap and turn-scoped.
- Append-only history is enforced for accounts created on or after 2026-08-31 (400 on prefix mismatch, or `drop_block` with `thinking-binding-controls-2026-08-01`). Edits that break it are the same ones that break the cache: injecting and removing per-turn reminders, summarizing in place, changing the system prompt mid-session. Fix: turn-scoped system messages, mid-conversation system messages, server-side compaction or context editing, or client compaction that replaces the whole history with one summary.
- Cache reads are 4x cheaper ($1.00 to $0.25 per MTok), so compacting early to save cost may no longer be the right tradeoff. Experiment with later compaction points.
- Safeguard false positives: ask "Are there any bugs in this program?" rather than "Does this compile?"; give docs for obscure languages; strip base64 from tool output.
- Subagents: make spawn return immediately, deliver results as later user messages, give a separate wait tool. The model still often waits; savings come from the runs where it carries on.
- Vision: a crop-and-enlarge tool alone gets most of the uplift.

### 3.4 Lance Martin threads

Tips thread (2026-09-01):

- Fable 5.1 at `low` effort is at parity with Fable 5 at `high` on CursorBench 3.2.0 at a third of the cost. The attached chart shows Fable 5.1's `low` point (~66%, ~$3/task) sitting above Opus 5 `high` and Fable 5 `medium`.
- Cache reads $1.00 to $0.25 per MTok. Claude Console shows cache hit rate. `/claude-api cost-optimize` diagnoses cache config.
- "Simplify your prompts | remove verification rituals, emphasis boosters, scratchpad scaffolds, stale few-shot examples, or contradictory rules." `/claude-api prompt-audit` inspects prompts or skills.
- Effort can change mid-conversation without breaking the cache (previously it invalidated).
- `/claude-api migrate` updates API configuration.

Prompt-audit thread (2026-09-02), the six patterns:

1. Verification rituals. "double-check your work" / "verify twice before responding" are taken literally and waste tokens.
2. Thoroughness and emphasis boosters. "Be maximally thorough," "CRITICAL: YOU MUST ALWAYS…" lead to verbosity and extra tool calls.
3. Mandatory procedures and scratchpad scaffolds. Fixed step processes or reasoning templates stack on top of native reasoning.
4. Stale examples. Few-shot examples tuned to an older model's failure modes teach imitation of long reasoning chains on requests that don't need them.
5. Contradictory rules. Followed more literally now; "always refund within policy" vs "never issue refunds without escalation" degrades performance.
6. Dated configuration. Old settings (manual thinking budgets) get rejected.

"a common reason is the frontier models are better at instruction following, so these anti-patterns steer them to spend unnecessary tokens." Measured example on an internal support benchmark: Opus 4.8 legacy prompt 89.4% at 2.52 cents per ticket; Opus 5 with the same prompt 91.7% at 3.43 cents; Opus 5 with audited prompt 97.0% at 2.93 cents. The audit step alone: +5.3 points, -0.49 cents. The skill is open source at github.com/anthropics/skills and "some of this guidance likely applies generally across frontier models."

---

## 4. The prompt-audit checklist (from the bundled `claude-api` skill)

This is the full mechanism behind `/claude-api prompt-audit`. It is the most detailed and best-argued source in the set, so the structure is preserved.

### 4.1 Frame

"The audit's job is therefore to find specific dated instructions, not to make prompts shorter. 'Every token earns its place' is the frame; 'make it short' is not." An audit that finds nothing should change nothing.

Provenance question for every emphatic or prohibitive line: "which failure, on which model, did this prevent - and does that failure still reproduce on the target model?" Use `git blame`. Idiom-dating alone (scratchpad tags, "think step by step", prefills, ROLE/CONTEXT/RULES/EXAMPLES boilerplate) is low confidence; pair it with a documented model behaviour to raise it.

The deletion rule: "could the model already know this?"

- Keep what only the author knows: audience and product, environment facts, the quality bar, tool contracts and mechanics, genuinely hard judgment calls, and the reasons behind constraints. "This is context, and context is never cruft."
- Candidates for removal: restatements of trained defaults, behaviour the model already does unprompted (thoroughness, planning, tool use), workarounds for failures the target model no longer has.
- Sharpening question: is the line a constraint on behaviour (test it) or context the model can't get elsewhere (keep)? "A naive shortening pass deletes exactly the highest-value words."

### 4.2 Group 1: dated prompt text

**1a. Pressure language.** Say exactly what you mean at normal volume. Cuts both ways: inflated emphasis over-triggers; leftover hedges ("try to", "if possible") are now read literally as permission to under-deliver.

| Before | After |
|---|---|
| `CRITICAL: You MUST use this tool when...` | `Use this tool when...` |
| `IMPORTANT: NEVER do X` (several per prompt) | State the one or two real constraints plainly, with the reason |
| `If in doubt, use [tool]` / `Default to [tool]` | delete, or `Use [tool] when it would improve X` |
| `Be thorough. Do not be lazy. Do not stop early.` | delete |
| `Try to include a summary if possible` (when required) | `Include a summary.` |
| `You have a tendency to over-X` / `Don't be too verbose` | State the desired behaviour |

"An anxious prompt produces a cautious, hedging model. Emphasis is not banned; it is a tested, scoped fix for one demonstrably underweighted instruction, not a first-draft register."

Grep signals: density of `MUST|NEVER|ALWAYS|CRITICAL|IMPORTANT` in caps; `!!`; emphasis with no adjacent "because"; `try to|if possible|ideally` on real requirements; `you (tend to|often|sometimes)`; `don't be too [adjective]`.

**1b. Scaffolds replaced by API features.** "Think step by step" and scratchpad tags become adaptive thinking plus effort. "Plan before acting" gets deleted (causes over-planning; lower effort instead). "Show your thinking" reads thinking blocks via the API and on Fable 5.1 can trigger a reasoning-extraction refusal. Prefill plus JSON-forcing stack becomes structured outputs (and the surrounding retry code is cruft too). "Summarize progress every N tool calls" and hard word caps get deleted and re-baselined; "output caps starve reasoning on hard problems." Inline lookup tables and arithmetic rubrics move to files or code. Forced `tool_choice` becomes a prompt instruction under `auto` (400 on Fable 5.1).

**1c. Over-specification.** Describe the goal, not the method.

| Pattern | Why cruft | Fix |
|---|---|---|
| Step-by-step choreography for judgment tasks | Too prescriptive; the model's own plan usually beats a hand-written script | State outcomes, constraints, and how to verify; numbered steps only where order truly matters |
| Prohibition lists | "a prohibition against a failure the model wasn't going to make can anchor it toward that failure" | Keep prohibitions whose failure reproduces; rewrite the rest as positive intent |
| The single gold example; stale few-shot | Examples are the strongest signal; the model matches their length, tone, structure, and old examples freeze old behaviour | Several varied examples labelled illustrative; keep only those pinning a format-sensitive shape |
| Bullet walls for behavioural guidance | "Bullets flatten priority and sever rules from reasons, and prompt format bleeds into output format" | Structure for reference data; prose for behaviour, carrying the "because" |
| Padding: generic virtues, repetition, kitchen-sink edge cases | Everything is treated as actionable; duplicated rules cost reconciliation effort; bulk inflates thinking spend | Say it once, in the right place |
| Grader vocabulary ("you will be graded on") | Pushes effort toward being-watched | State every requirement; never describe the grader |
| Strategy coaching ("it's usually best to...") | Author heuristics are wrong somewhere; the model's plan is usually better | "If removing the sentence wouldn't change what is legal or how success is measured, it's strategy - delete it" |

**1d. Fossils.** Model-version workarounds (trace each to its model; retired model means remove and re-test). Migration-relative phrasing ("now works differently", "no longer") is "a diff against a previous prompt version the model never saw"; write as if current rules always existed. Patch accretion of narrow conditionals: generalize the principle; test removals, not just additions. Unenforced instructions: enforce in code what can be, delete what nothing checks. Identity stubs substituting for context. Update suppressors ("hold all findings", "don't narrate") now cause under-narration on Fable 5.1. Anti-formatting rules now strip formatting the reader wanted. Cadence reminders re-inserted in the harness are a retention crutch and a history edit under preserved thinking.

**1e. Prohibition clusters.** Judge by provenance, not need. "Does it carry a stated reason or encode a real business/policy constraint?" Reasoned prohibitions stay with their reason beside them. Style bans with no provenance (banned phrases, tic lists) become one positive line. "A surrounding cluster of legitimate reasoned prohibitions does not launder the no-provenance ones mixed into it."

**1f. Output-shaping choreography.** Cadences, numeric ceilings, and cut-the-detail lines are one pattern; remove every limb together. "A stated operational reason ... does not convert a numeric clamp into a keeper." Re-express as audience/outcome framing without the number.

### 4.3 Group 2: brittle skill files

"Skill size is a tax paid on every trigger."

| Pattern | Why cruft | Fix |
|---|---|---|
| Verbose SKILL.md explaining what the model knows | Every paragraph must justify its cost | Apply the deletion rule paragraph by paragraph |
| Wrong degrees of freedom | Exact scripts over-constrain judgment; vague prose under-constrains fragile ops | Match specificity to fragility: prose for open fields, exact commands (`do not modify this command`) only for narrow bridges |
| The recency trap: one session's stumble as a permanent rule | The next session steps around a pothole that isn't there | "would this have helped most recent sessions, or just the one that wrote it?" |
| Volatile specifics: paths, flags, versions, unverified API claims | Skills rot factually | Encode architecture, data models, workflows; verify surviving facts against current code |
| Time-sensitive content, option menus, duplicated info across files | Dates rot; menus dilute; duplicates drift | An "old patterns" section instead of dates; one default plus an escape hatch; one place per fact |
| History narratives: past tense, incident IDs, PR numbers, pinned model names | Authority is the behaviour prescribed, not the incident | State the current rule; drop the archaeology |
| Trigger-case enumeration in descriptions, growing one phrase per missed trigger | Descriptions ride in every request; enumeration generalizes worse than intent categories | Name generalized categories of intent |

Signals: SKILL.md not readable in one sitting; hardcoded paths and version pins; past tense in instruction files; descriptions that only ever grow in git history.

### 4.4 Group 3: tool descriptions

"The rubric for tool descriptions is precision and contract accuracy, not brevity." The most common failure is under-description. Contract and mechanics in; behavioural steering and worked examples out. A description is a man page: what it does, when to use it and when not, parameters, caveats, what it does not return.

- Vague one-liners, undocumented parameters, no when-not-to-use: add. Three to four sentences minimum.
- `CRITICAL: You MUST use this tool when...`: dial back to `Use this tool when...`.
- Worked examples, fake dialogue, embedded protocols in descriptions: move to skills or progressive disclosure; make parameters expressive (well-named enums carry intent).
- Scolding cross-references (`ALWAYS use X, NEVER use Y`) and behaviour-smuggling: put a preference for X in X's description, not scattered across rivals.
- Tool names in the system prompt: delete; then toggling a tool never leaves a dangling reference.
- Overlapping tools, bloated payloads, 30+ always-loaded tools: fewer, bounded tools; tool search or deferred loading past a few dozen.

**Trigger text is not behavioural text.** A skill's frontmatter description or trigger block "may legitimately carry calibrated urgency, because skills currently under-trigger; ideally it's tuned against a trigger eval rather than vibes. Text whose job is behavior should explain rather than shout. These look identical to a grep, so classify by function before flagging."

### 4.5 Group 4: request config and architecture

API fossils; cache-hostile ordering (timestamps and IDs above stable content); budget countdowns rendered into context cause premature wrap-up; an LLM executor for a deterministic plan (count model-call sites, move routing/tallying/formatting back to code, keep exactly one adaptive call); redundant specialist sub-agents (fold the one real difference into the survivor as an input); no token accounting (prerequisite for measuring any cleanup).

### 4.6 The keep list

1. Context is never cruft. "Too-short prompts produce generic output because the model fills gaps with safe defaults; give the model more context than seems necessary, not less."
2. Cruft is not length. Never justify a deletion by character count.
3. Fragile operations keep exact scripts (destructive commands, auth flows, compliance steps). "Prompting effort should scale with how far the task is from what the model does naturally."
4. Tool contract detail stays and often grows.
5. Prohibitions against current, demonstrated failures stay.
6. Trigger/routing text may carry calibrated urgency.
7. Format-pinning examples on format-sensitive outputs stay, labelled illustrative.
8. Working redundancy is not cruft. Dedupe only when duplicates disagree.
9. A one-line role statement is fine.
10. Deliberate end-of-prompt recap of a few key constraints is fine; scattered duplication is not.
11. Re-baselining adds text too: new models have new failure modes that need new guidance. "The audit's job is fit, in both directions."

### 4.7 Process discipline

Report each finding as location, quoted evidence, pattern row, why obsolete for the target model, confidence (high = documented or errors; medium = widely observed; low = idiom-dating, flag only), action (`remove` / `rewrite` with replacement / `move` / `replace-with-API-feature` / `add` / `flag`). One finding per diff hunk. "Rewrites beat bare deletions where the instruction has a live purpose." Verify by probing behaviour before and after on a scratch copy; "asking the model whether it needs an instruction is not a measurement." If a cut regresses, re-add in minimal form, not the verbose original. Grep the wider system for exact prompt strings before deleting (classifiers, tests, log parsers). Re-audit at every model release.

---

## 5. Consolidated principles for skill and AGENTS.md writing

Pulled together across all sources, ordered by how strongly the sources agree.

1. **Describe outcomes and constraints; drop the recipe.** Numbered steps only where order is fragile or exactly one sequence is safe. Prose heuristics for judgment; exact commands only for narrow bridges.
2. **Say it once, at normal volume, with the reason.** No CRITICAL/ALWAYS/NEVER stacks. Emphasis is a scoped fix for one measured under-trigger, and belongs in trigger text more than body text.
3. **Prompt the positive.** Prohibitions with no provenance become one line of desired behaviour. Prohibitions that encode a real constraint stay, with the reason beside them.
4. **Define completion before starting.** Say what "done" includes (run it, inspect it, fix failures) and where to stop. A requirement to stop for review pulls the model to an earlier stop; check that you actually need that gate.
5. **Grant autonomy explicitly for known-safe workflows.** Both models take boundaries seriously and will stop where you'd rather they continue. Name the safe workflow and remove per-step approval. Keep destructive and irreversible actions gated.
6. **User instructions beat skill instructions.** Say so, and give the model a way to blame the skill line that made it pause.
7. **Skill descriptions: short, trigger-precise, one branch per trigger.** Descriptions are always-loaded and get truncated when there are many skills. Name intent categories, not synonym lists. "Pick me" energy misroutes.
8. **Root document as router; branches disclosed behind pointers.** Reading a skill costs context and drags in guidance that may not apply. Point contextually ("use X.md for schema changes"), never unconditionally ("read X before every edit").
9. **Match tests to the change.** Both models over-test small changes. Tests where the task asks or the repo convention exists, sized like neighbours; scratch checks stay scratch.
10. **Scope is the deliverable.** No quiet narrowing, widening, or swapping. Side findings go in the summary as follow-ups, not into the diff.
11. **Formatting: say when, not never.** Astra needs a lean toward prose; Fable 5.1 needs permission to use structure. Anti-formatting rules are fossils on Fable 5.1.
12. **Kill the fossils.** Model-name pins, incident narratives, "no longer"/"now" phrasing, cadence reminders, "hold all findings", "double-check twice", scratchpad tags, "think step by step".
13. **Keep context, contracts, and reasons.** Environment facts, unwritten conventions, gotchas no config confesses, tool parameter semantics and failure modes. Under-described tools are the more common failure than over-described ones.
14. **Volatile specifics belong in the environment.** Paths, flags, versions rot; `--help` and config files do not.
15. **Write for the multi-model audience.** A repo skill is read by Astra, Fable, Sol, Luna. Guidance tuned to one model's weakness over-constrains another.
16. **Verify by behaviour, one change at a time.** Re-audit on every model release.

---

## 6. Leads for the `but` skill (to be worked later)

Not conclusions, just where to look first when applying the above to `crates/but/skill/SKILL.md` and the agent-setup managed blocks.

- Run both audits on the skill and its managed blocks: `/claude-api prompt-audit crates/but/skill` and Brodin's AGENTS.md review prompt. Compare findings.
- Check the description against the Provencher test: does it fire only on version-control work, or on anything git-adjacent? Count its trigger synonyms; collapse to one per real branch.
- Look for recipe-shaped sections (numbered workflows for judgment work) versus fragile bridges (exact command syntax, destructive ops) and re-tier them per the degrees-of-freedom rule.
- Hunt fossils: retired-syntax hints, "no longer"/"now" phrasing, model or version pins, incident narratives.
- Check completion definitions: does "commit" or "ship it" define where the agent should stop, and does anything force an unnecessary review gate?
- Check that tool-contract detail (command semantics, ID formats, what `--status-after` does and doesn't return) is complete rather than shouted.
- Kiril's global rules file still carries retired `but commit` syntax and several ALWAYS/NEVER lines; it is in the same audit scope.

---

## 7. Case study: agent-browser

Moved to [agent-browser-case-study-2026-09.md](agent-browser-case-study-2026-09.md): skill served from the binary behind a thin installed stub, a regex-plus-rubric eval suite for the skill itself, and the CLI surface decisions (handle lifecycles, error codes with recovery data, truncation trailer, session helper, guardrails in the tool).
