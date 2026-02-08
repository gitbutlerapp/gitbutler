# Skill File Evaluation Research

How do we know if our skill file is "good"? How do we prove changes improve it? This document synthesizes industry research into a concrete strategy for quantitatively testing the `but` CLI skill.

## The Problem

Every change to `SKILL.md` is gut-feel based. We can read it and think "this seems clearer," but we have no way to measure whether agents actually behave better after a change. We need:

1. **Metrics** — What to measure
2. **Test cases** — What scenarios to test
3. **Infrastructure** — How to run tests reproducibly
4. **Regression tracking** — How to know if a change helped or hurt

## Current Direction

As of February 7, 2026, this project is operating in a **Tier 4-first** mode:

- Tier 4 integration tests are the main source of truth for skill quality
- Tier 1/2/3 are secondary tools for diagnostics and faster iteration
- A Tier 4 smoke run is expected for skill-file changes

## Industry Landscape

### How Others Test Agent Tool Use

**Berkeley Function-Calling Leaderboard (BFCL)** — The most comprehensive public benchmark for LLM tool use. Evaluates across dimensions: tool selection (did it pick the right tool?), parameter accuracy (right arguments?), relevance detection (correctly refusing irrelevant tools), and multi-turn chaining. Uses AST comparison of generated function calls against ground truth.

**SWE-bench** — Tests end-to-end coding agent performance on real GitHub issues. Binary pass/fail based on whether the agent's patch passes the test suite. Key insight: test the *outcome* (did the task succeed?), not just individual tool calls.

**MCP-Bench** — Benchmarks tool-using LLM agents across tool appropriateness (right tool for the subtask) and parameter accuracy (correct/complete arguments). Uses prompt shuffling to control for ordering bias in evaluation.

**DeepEval Tool Correctness** — Deterministic metric: `correctness = correctly_used_tools / total_tools_called`. Evaluates tool selection, input parameters, and output accuracy. Can layer LLM-as-judge for optimality assessment.

### How Anthropic Recommends Testing Skills

From Anthropic's [skill authoring best practices](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices):

> **Create evaluations BEFORE writing extensive documentation.** This ensures your Skill solves real problems rather than documenting imagined ones.

Their recommended evaluation-driven development loop:

1. **Identify gaps** — Run Claude on representative tasks *without* a skill. Document specific failures
2. **Create evaluations** — Build scenarios that test these gaps
3. **Establish baseline** — Measure performance without the skill
4. **Write minimal instructions** — Only enough to pass evaluations
5. **Iterate** — Run evals, compare against baseline, refine

Evaluation structure they suggest:
```json
{
  "skills": ["but"],
  "query": "Commit the auth changes to the feature branch",
  "files": ["src/auth.rs"],
  "expected_behavior": [
    "Runs but status --json to check workspace state",
    "Uses but commit with --changes flag for specific files",
    "Includes --json and --status-after flags"
  ]
}
```

They also recommend testing with all model tiers (Haiku, Sonnet, Opus) since skill effectiveness varies by model capability.

### Available Frameworks

| Framework | Type | Strengths | Fit for Us |
|-----------|------|-----------|------------|
| **[promptfoo](https://www.promptfoo.dev/)** | OSS, YAML-driven | Native Claude Agent SDK provider, tool use assertions, `--repeat N` for statistics, HTML reports, cost tracking | **Best fit for Tier 4-first harness (and Tier 2-3 when needed)** |
| **[Braintrust](https://www.braintrust.dev/)** | Commercial | Trajectory scoring, experiment comparison, tracing | Overkill for now |
| **[inspect_ai](https://inspect.ai-safety-institute.org.uk/)** | OSS, Python | Docker sandboxes, multi-turn agent eval, solver/scorer separation | Optional for large-scale secondary evals |
| **[DeepEval](https://github.com/confident-ai/deepeval)** | OSS, pytest | Tool Correctness metric, G-Eval custom criteria | Good metrics reference |
| **Claude Agent SDK** | Anthropic | `claude -p` headless mode, `--output-format json`, structured output | **Essential for Tier 4** |

### promptfoo + Claude Agent SDK Integration

promptfoo has a dedicated Claude Agent SDK provider that runs agents with full tool access:

```yaml
providers:
  - id: anthropic:claude-agent-sdk
    config:
      model: claude-sonnet-4-5-20250929
      working_dir: ./test-repo
      append_allowed_tools: ['Bash', 'Read', 'Edit']
      permission_mode: acceptEdits
      max_turns: 10
      max_budget_usd: 0.25
      ask_user_question:
        behavior: first_option  # Auto-answer prompts

tests:
  - vars:
      prompt: "Commit my auth changes"
    assert:
      - type: contains
        value: "but commit"
      - type: not-contains
        value: "git commit"
      - type: cost
        threshold: 0.25
      - type: javascript
        value: |
          const text = String(output).toLowerCase();
          return text.includes('--json') && text.includes('--status-after');
```

## Metrics That Matter

Based on industry standards and our specific skill file, here are the metrics to track:

### Core Metrics

| Metric | What It Measures | Target | How to Score |
|--------|-----------------|--------|-------------|
| **Tool routing accuracy** | Uses `but` instead of `git` for write ops | 100% | Binary per command |
| **`--json` compliance** | All `but` commands include `--json` | 100% | Count across all commands in response |
| **`--status-after` compliance** | Mutation commands include `--status-after` | 100% | Check commit/absorb/rub/stage/squash/move/uncommit |
| **`--changes` specificity** | `but commit` uses `--changes` with explicit IDs (`a1,b2` or repeated flag), not bare commit | >90% | Binary per commit command |
| **Workflow ordering** | Runs `but status --json` before mutations | 100% | Check command sequence |
| **Unnecessary round-trips** | No `but status` after commands with `--status-after` | 0 | Count redundant status calls |
| **Task completion** | End-to-end task succeeds | >80% | Binary per scenario |

### Derived Metrics

| Metric | Formula |
|--------|---------|
| **Instruction compliance rate** | instructions_followed / instructions_relevant_to_task |
| **Tool efficiency** | expected_tool_calls / actual_tool_calls |
| **Common mistake avoidance** | 1 - (documented_mistakes_triggered / tasks_run) |
| **Cost per task** | Total tokens consumed (input + output) |

### What the Industry Measures (for reference)

From the academic survey literature:

- **Invocation accuracy** — Correct decision on whether to call a tool at all
- **Tool selection accuracy** — Correct tool from available set
- **Parameter accuracy** — Correct argument names and values
- **Hallucination rate** — Invented tools or parameters
- **Steps to completion** — Number of tool calls to finish
- **Error recovery rate** — Recoveries / errors encountered

## Testing Architecture

### Tier 1: Static Analysis (Zero-cost, instant)

Validate skill file structure without calling any LLM. Run as part of `cargo test`.

**What to check:**
- YAML frontmatter is valid and meets Anthropic's constraints (name <=64 chars, description <=1024 chars)
- All referenced files exist (`references/reference.md`, etc.)
- Code examples are internally consistent (every mutation command example includes `--json --status-after`)
- No contradictions between SKILL.md and reference files
- Translation table covers all commands mentioned in reference.md
- Line count stays under 250 (our budget)
- Description field contains key trigger words

**Implementation:** Rust tests in `crates/but` or a simple script that parses SKILL.md.

### Tier 2: Single-Turn Tool Selection (Low-cost, fast)

Test whether Claude picks the right first command given a user prompt and the skill file as context. Uses the Anthropic API with mock tool definitions.

**Pattern:**
```
Input: skill_file + user_prompt → Model → First tool call
Score: Does the tool call match expectations?
```

**Example test cases:**

| User Prompt | Expected First Command | Assertions |
|------------|----------------------|------------|
| "What files have I changed?" | `but status --json` | contains `but status`, contains `--json` |
| "Commit my auth changes" | `but status --json` | status first, then commit with `--changes` |
| "Create a new branch for auth" | `but branch new auth` | contains `but branch new` |
| "Push my changes" | `but push` | NOT `git push` |
| "Squash my last 3 commits" | `but squash` | NOT `git rebase -i` |
| "Can you do a git push?" | `but push` | uses `but` not `git` |
| "Check what's changed" | `but status --json` | NOT `git status`, NOT `git diff` |
| "Undo my last commit" | some `but` command | NOT `git reset` |

**Implementation options:**

1. **promptfoo** (recommended) — YAML config, built-in assertions, HTML reports
2. **Custom Python script** — Maximum control, use `anthropic` SDK
3. **Claude CLI** — `claude -p "prompt" --output-format json`

### Tier 3: Multi-Turn Workflow (Medium-cost, comprehensive)

Test complete workflows with mock tool execution. This is the highest-signal tier.

**Pattern:**
```
1. Provide skill file as system context
2. Send user prompt
3. Model returns tool call → feed mock result
4. Model returns next tool call → feed mock result
5. Repeat until model finishes
6. Score the entire command sequence
```

**Example scenarios:**

**Scenario: Basic commit flow**
```
User: "I just finished implementing auth. Commit it."
Expected sequence:
  1. but status --json           (check state)
  2. but commit <branch> -m "..." --changes <id>,<id> --json --status-after  (commit)
Assertions:
  - Step 1 happens before step 2
  - Commit includes --changes (not bare commit)
  - Commit includes --json --status-after
  - No git commands used
```

**Scenario: New feature workflow**
```
User: "Add a dark mode feature"
Expected sequence:
  1. but status --json           (check state)
  2. but branch new dark-mode    (create branch)
  3. [file edits happen]
  4. but commit ... --changes ... --json --status-after
Assertions:
  - Branch created before any commits
  - Commit targets the new branch
```

**Scenario: Multiple independent features**
```
User: "Add API endpoint and update UI styling"
Expected:
  - Two parallel branches created
  - Files staged to appropriate branches
  - Separate commits per branch
```

**Mock tool execution:**

The key insight from the research: you don't need to run real commands. Mock the tool results:

```python
def mock_bash(command: str) -> str:
    if "but status --json" in command:
        return json.dumps({
            "unassignedChanges": [
                {"cliId": "a1", "filePath": "src/auth.rs", "changeType": "modified"}
            ],
            "stacks": [{"cliId": "su", "branches": [{"cliId": "bu", "name": "main"}]}]
        })
    if "but commit" in command:
        return json.dumps({"result": {"commitId": "abc123"}, "status": {...}})
    if "but branch new" in command:
        return json.dumps({"result": {"branchId": "bv", "name": "..."}})
    return "unknown command"
```

This gives full control, deterministic scoring, and low cost (can use Sonnet/Haiku).

#### Tier 3 Implementation Notes

**Key insight: Tier 3 tests the skill file, not the `but` CLI.** No `but` binary runs. No git repo exists. The mock handlers return canned JSON that looks like `but status --json` output. You're measuring whether SKILL.md *teaches the model correctly* — complementary to Tier 1's structural validation.

```
                    ┌─────────────┐
  SKILL.md ───────► │  LLM (API)  │ ◄──── user prompt
  (system context)  └──────┬──────┘
                           │
                      tool_use: "but status --json"
                           │
                    ┌──────▼──────┐
                    │ Mock handler │ ──► canned JSON
                    └──────┬──────┘
                           │
                      tool_use: "but commit ... --changes a1 --json --status-after"
                           │
                    Score: did the command sequence follow SKILL.md rules?
```

Tier 3 remains useful for cheap, deterministic diagnostics, but this project gates on Tier 4 integration.

**Current Tier 4 scenario set (7 scenarios):**

| # | Scenario | Key assertions |
|---|----------|----------------|
| 1 | Basic commit flow | `status --json` before `commit`; commit has `--changes`, `--json`, `--status-after`; no git write commands |
| 2 | Branch workflow | Create branch (`but branch new` or `but commit <branch> -c`) before committing |
| 3 | Git synonym redirect | User says "git push", model uses `but push` and not `git push` |
| 4 | Ordering flow | `but status --json` occurs before `but commit` |
| 5 | Specificity flow | Single-file commit uses `--changes`; non-target file remains unassigned in repo state |
| 6 | Amend flow | Use `but amend` with `--json --status-after`; no git write fallback |
| 7 | Reorder flow | Use `but move`/`but rub` with `--json --status-after`; no `git rebase`/checkout fallback; repo reflects target order |

### Tier 4: Integration (High-cost, realistic)

Run Claude Code against a real test repository with the latest `but` binary and skill files. Unlike Tier 3's mocks, this tests the full stack: skill file → agent behavior → actual CLI execution → real repo state changes.

**What makes this different from Tier 3:**

| | Tier 3 (mock) | Tier 4 (integration) |
|---|---|---|
| Runs `but` binary | No | Yes — freshly built from source |
| Real git repo | No | Yes — disposable fixture |
| Command trace | From mock loop | From SDK hooks or output parsing |
| Asserts on repo state | No | Yes — `but status --json` after |
| Cost per scenario | ~$0.02 | ~$0.10-0.50 |
| Speed | ~5 sec | ~30-120 sec |
| Catches real bugs | Skill file only | Skill + CLI interaction |

#### Current Harness Implementation

The current Tier 4 harness lives in `crates/but/skill/eval/` and uses:
- `providers/but-integration.ts` for real Agent SDK execution with Bash hook traces
- `promptfooconfig.yaml` for scenario data
- `assertions/but-assertions.ts` for shared assertion functions (`file://...:functionName`)
- `setup-fixture.sh` for disposable repo setup and skill installation

#### How to Run

```bash
cd crates/but/skill/eval

# One run (PR smoke)
pnpm run eval

# Repeated run (nightly/pre-release)
pnpm run eval:repeat

# View report UI
pnpm run view
```

#### Field Learnings (Observed February 7, 2026)

Running the real Tier 4 harness surfaced a few practical issues that are not obvious from design alone.

**Measured baselines from live runs:**
- `2026-02-07` eval `eval-5T4-2026-02-07T17:57:51`: **4/5 pass (80%)** on the initial suite.
- `2026-02-07` eval `eval-8oX-2026-02-07T19:26:09`: **1/7 pass (14.29%)** after adding stricter assertions plus amend/reorder scenarios.
- `2026-02-07` eval `eval-qCp-2026-02-07T19:28:35`: **2/7 pass (28.57%)** after iterative scenario tuning.
- `2026-02-07` eval `eval-lG2-2026-02-07T19:32:49`: **2/7 pass (28.57%)** after adding repo-state ordering validation for reorder.

**What this indicates:**
- Most current failures are real policy gaps, not harness bugs: the agent still leaks into raw `git` writes (`git add`, `git commit`, `git push`, `git checkout`) under several prompts.
- Amend and reorder scenarios are representative for `but` because they exercise explicit history-editing commands (`amend`, `move`/`rub`) where fallback behavior is currently weak.

**Implementation fixes required for stable Tier 4 runs:**

1. **promptfoo `javascript` assertion return shape is version-sensitive.**
   - In `promptfoo` 0.119.x, returning `{ pass, reason }` from `type: javascript` caused failures.
   - Returning a plain boolean stabilized assertions across runs.

2. **Do not mutate `process.env` globally in a concurrent provider.**
   - Parallel test execution caused cross-test races when setting `E2E_TEST_APP_DATA_DIR` globally.
   - Fix: pass a per-invocation `env` object into SDK query/options and child processes.

3. **Canonicalize fixture paths before `but setup`.**
   - `but setup` stored `/private/var/...` while later status checks used `/var/...`, causing "Setup required" lookup failures.
   - Fix: normalize fixture path with `pwd -P` in `setup-fixture.sh`.

4. **Keep fixture support files out of Git status.**
   - `.but-data/` and installed `.claude/skills/` content polluted `but status --json` and changed CLI IDs.
   - Fix: add `.but-data/`, `.claude/`, `.tmp/` to `.git/info/exclude` in each fixture.

5. **Fixture cleanup should be best-effort.**
   - Rare `ENOTEMPTY` races during directory deletion can fail otherwise-successful evals.
   - Fix: treat cleanup errors as non-fatal in provider `finally` blocks.

**Behavioral takeaway:**
- Tier 4 correctly catches real regressions that Tier 3 mocks can miss (for example, model fallback to raw `git push` despite skill intent).
- Keep the git-synonym scenario as a required gate for future skill revisions.

#### Cost and Cadence

- ~$0.10-0.50 per scenario (real Claude Code turns against a real repo)
- Full suite of 7 scenarios × 3 repeats is roughly in the same order of magnitude (typically low single-digit USD to low double-digit USD depending on turn count)
- PRs touching `crates/but/skill/`: run Tier 4 smoke once (`--repeat 1`)
- Nightly or pre-release: run Tier 4 with repeats (`--repeat 3` or higher)
- Keep Tier 2/3 as supplemental diagnostics, not primary gates

#### Why TypeScript, Not Rust

This project is a Rust codebase, but Tier 4 evals are TypeScript. The reason is practical:

- The **Claude Agent SDK** (which provides `PostToolUse` hooks for command trace capture) only exists in TypeScript and Python — there is no Rust SDK
- Without hooks, you're limited to `std::process::Command::new("claude").arg("-p")` which gives you final output but no command trace — strictly less informative
- The **promptfoo** ecosystem (YAML configs, `--repeat`, HTML reports, assertions) is JS/TS-native
- The eval harness is ~80 LOC of glue code calling external processes — Rust's strengths (performance, safety) don't apply here

For **Tier 3** (mock tool execution), Rust is viable since it just calls the Anthropic API directly — no Agent SDK needed. The tradeoff is losing promptfoo's reporting infrastructure. For Tier 4, TypeScript is the clear choice.

## Recommended Strategy

### Tier 4-First Rollout

1. **Keep Tier 4 as the default evaluator** for skill changes.
2. **Treat a 7-scenario Tier 4 smoke run (`--repeat 1`) as the PR gate** for changes under `crates/but/skill/`.
3. **Run repeated Tier 4 (`--repeat 3+`) nightly or pre-release** to catch stochastic regressions.
4. **Track the key Tier 4 metrics over time**: pass rate, git-command leakage rate, `--json` and `--status-after` compliance, and cost per scenario.

### Supplemental Layers (Optional)

1. **Use Tier 2/Tier 3 only for targeted debugging** when Tier 4 fails and root cause is unclear.
2. **Use Tier 1 static checks as cheap hygiene**, not as confidence signals for behavior.
3. **Retain at least one git-synonym scenario** (`"git push"`) as a hard regression detector.

### Cross-Model Policy

1. **Primary model for gating**: Sonnet (current operational model).
2. **Secondary model sweeps**: Haiku/Opus on-demand when major skill rewrites land.
3. **Do not block on secondary models by default** unless product policy requires it.

## Framework Recommendation

**Use promptfoo + custom provider as the primary Tier 4 harness.** Reasons:
- YAML configuration (low barrier, version-controllable)
- Built-in assertions (`contains`, `not-contains`, `javascript`, `llm-rubric`, `cost`, `latency`)
- `--repeat N` for statistical significance
- HTML report generation
- Can run from CLI or CI
- Open source

**Important nuance:** promptfoo's vanilla `anthropic` provider is single-turn and insufficient for workflow traces. For Tier 4, prefer a **custom provider** (`file://dist/providers/but-integration.js`) that wraps Claude Agent SDK hooks and captures Bash command traces.

**Use `promptfoo eval` with the custom provider** for standard runs, and `claude -p` only for ad-hoc reproduction.

## Scoring Rubric

For each test, produce a composite score:

| Dimension | Weight | Scoring |
|-----------|--------|---------|
| **Correctness** | 40% | Did the task complete? (binary) |
| **Tool compliance** | 30% | Fraction of commands with correct tool + flags |
| **Efficiency** | 15% | normalized by expected tool calls (penalize >2x expected) |
| **Safety** | 15% | No dangerous commands (git writes, bare absorb) |

**Aggregate skill score** = weighted average across all test cases.

Track this score over time. Every SKILL.md change should improve or maintain the score.

## Statistical Considerations

LLM outputs are non-deterministic. To get meaningful signal:

- **Run each test at least 3-5 times** (promptfoo `--repeat` flag)
- **Use temperature 0** for eval runs to reduce variance
- **Report mean + std deviation** for each metric
- **Need ~30 runs per variant** at temperature>0 for A/B testing
- **Watch for prompt sensitivity** — small wording changes can have outsized effects

## Key References

- [Anthropic Skill Authoring Best Practices](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices)
- [Anthropic Advanced Tool Use](https://www.anthropic.com/engineering/advanced-tool-use)
- [Claude Code Headless Mode / Agent SDK CLI](https://code.claude.com/docs/en/headless)
- [Claude Code CLI Reference](https://code.claude.com/docs/en/cli-reference) — full flag reference (`--max-turns`, `--max-budget-usd`, `--allowedTools`, `--output-format`, etc.)
- [Claude Agent SDK Overview](https://platform.claude.com/docs/en/agent-sdk/overview) — TypeScript/Python SDK with hooks, sessions, permissions
- [Claude Agent SDK Hooks](https://platform.claude.com/docs/en/agent-sdk/hooks) — `PreToolUse`/`PostToolUse` for intercepting tool calls
- [promptfoo: Evaluate Coding Agents](https://www.promptfoo.dev/docs/guides/evaluate-coding-agents/)
- [promptfoo: Claude Agent SDK Provider](https://www.promptfoo.dev/docs/providers/claude-agent-sdk/)
- [promptfoo: Custom Providers](https://www.promptfoo.dev/docs/providers/custom-api/) — `callApi()` interface for custom execution loops
- [Berkeley Function-Calling Leaderboard](https://gorilla.cs.berkeley.edu/leaderboard.html)
- [DeepEval Tool Correctness](https://deepeval.com/docs/metrics-tool-correctness)
- [LLM Agent Evaluation Survey (KDD 2025)](https://dl.acm.org/doi/10.1145/3711896.3736570)
- [Claude's Context Engineering Secrets](https://01.me/en/2025/12/context-engineering-from-claude/)
- [SWE-bench](https://www.swebench.com/)
- [MCP-Bench](https://www.arxiv.org/pdf/2508.20453)
