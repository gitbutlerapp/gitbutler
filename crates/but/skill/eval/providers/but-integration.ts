import { execFileSync, execSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { query, type HookInput, type SDKResultMessage } from "@anthropic-ai/claude-agent-sdk";

type ProviderConfig = {
  model?: string;
  max_turns?: number;
  max_budget_usd?: number;
  repo_root?: string;
  but_bin?: string;
  keep_fixtures?: boolean;
  allowed_tools?: string[];
};

type PromptfooContext = {
  vars?: Record<string, unknown>;
};

type CommandTrace = {
  command: string;
  failed: boolean;
};

type RepoCommit = {
  cliId?: unknown;
  message?: unknown;
};

type RepoBranch = {
  name?: unknown;
  commits?: unknown;
};

type RepoStack = {
  branches?: unknown;
};

type RepoState = {
  stacks?: unknown;
};

const DEFAULT_ALLOWED_TOOLS = [
  "Bash",
  "Read",
  "Edit",
  "Write",
  "Glob",
  "Grep",
  "LS",
  "MultiEdit",
  "TodoWrite",
];

const GIT_WRITE_COMMAND_RE =
  /\bgit\s+(add|commit|push|merge|rebase|checkout|switch|stash|cherry-pick)\b/;
const GIT_ANY_COMMAND_RE = /\bgit\s+/;
const BUT_STATUS_RE = /^\s*but\s+status\b/;
const BUT_AMEND_RE = /\bbut\s+amend\b/;
const BUT_MOVE_RE = /\bbut\s+move\b/;

function hasRequiredMutationFlags(command: string): boolean {
  return command.includes("--json") && command.includes("--status-after");
}

function asRepoState(value: unknown): RepoState | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  return value as RepoState;
}

function branchesFor(state: RepoState, branchName: string): RepoBranch[] {
  if (!Array.isArray(state.stacks)) {
    return [];
  }
  const branches: RepoBranch[] = [];
  for (const stack of state.stacks as RepoStack[]) {
    if (!Array.isArray(stack?.branches)) {
      continue;
    }
    for (const branch of stack.branches as RepoBranch[]) {
      if (branch?.name === branchName) {
        branches.push(branch);
      }
    }
  }
  return branches;
}

function commitOrderMessages(state: RepoState, branchName: string): string[] {
  const branch = branchesFor(state, branchName)[0];
  if (!branch || !Array.isArray(branch.commits)) {
    return [];
  }
  return (branch.commits as RepoCommit[]).map((commit) =>
    typeof commit?.message === "string" ? commit.message : "",
  );
}

function commitCliIdByMessage(state: RepoState, branchName: string, messageNeedle: string): string | null {
  const branch = branchesFor(state, branchName)[0];
  if (!branch || !Array.isArray(branch.commits)) {
    return null;
  }
  for (const commit of branch.commits as RepoCommit[]) {
    const message = typeof commit?.message === "string" ? commit.message : "";
    const cliId = typeof commit?.cliId === "string" ? commit.cliId : null;
    if (cliId && message.includes(messageNeedle)) {
      return cliId;
    }
  }
  return null;
}

function parseJson(input: string): unknown {
  try {
    return JSON.parse(input);
  } catch {
    return null;
  }
}

function statusFromMutationOutput(output: string): RepoState | null {
  const parsed = parseJson(output);
  if (!parsed || typeof parsed !== "object") {
    return null;
  }
  const maybeStatus = (parsed as { status?: unknown }).status;
  return asRepoState(maybeStatus);
}

function isDesiredReorder(state: RepoState): boolean {
  const messages = commitOrderMessages(state, "reorder-test");
  const firstIndex = messages.findIndex((message) => message.includes("Add first.rs"));
  const secondIndex = messages.findIndex((message) => message.includes("Add second.rs"));
  return firstIndex >= 0 && secondIndex >= 0 && firstIndex < secondIndex;
}

function scriptDir(): string {
  return path.dirname(fileURLToPath(import.meta.url));
}

function evalDir(): string {
  let dir = scriptDir();
  for (let i = 0; i < 6; i += 1) {
    if (fs.existsSync(path.join(dir, "setup-fixture.sh"))) {
      return dir;
    }
    dir = path.resolve(dir, "..");
  }
  throw new Error("Could not locate eval directory containing setup-fixture.sh");
}

function fallbackRepoRoot(): string {
  return path.resolve(evalDir(), "../../../..");
}

function toMessage(error: unknown): string {
  if (error instanceof Error) {
    const maybeStdErr = (error as { stderr?: string | Buffer }).stderr;
    const stdErrText =
      typeof maybeStdErr === "string"
        ? maybeStdErr.trim()
        : Buffer.isBuffer(maybeStdErr)
          ? maybeStdErr.toString("utf8").trim()
          : "";
    return stdErrText ? `${error.message}: ${stdErrText}` : error.message;
  }
  return String(error);
}

function asCommand(toolInput: unknown): string | null {
  if (!toolInput || typeof toolInput !== "object") {
    return null;
  }
  const maybe = toolInput as { command?: unknown };
  if (typeof maybe.command !== "string") {
    return null;
  }
  const command = maybe.command.trim();
  return command.length > 0 ? command : null;
}

function stringEnv(overrides?: Record<string, string>): Record<string, string> {
  const entries = Object.entries(process.env).filter((entry): entry is [string, string] => typeof entry[1] === "string");
  return {
    ...Object.fromEntries(entries),
    ...overrides,
  };
}

function withButOnPath(env: Record<string, string>, butBin: string): Record<string, string> {
  const butDir = path.dirname(butBin);
  const existingPath = env.PATH ?? "";
  const mergedPath = existingPath.length > 0 ? `${butDir}${path.delimiter}${existingPath}` : butDir;
  return {
    ...env,
    PATH: mergedPath,
    BUT_BIN: butBin,
  };
}

function ensureGitButlerSetup(butBin: string, fixtureDir: string, env: Record<string, string>): void {
  try {
    execFileSync(butBin, ["-C", fixtureDir, "status", "--json"], {
      encoding: "utf8",
      env,
    });
  } catch (error) {
    throw new Error(
      `Fixture is not initialized for GitButler. Run 'but setup' before testing in this repo. ${toMessage(error)}`,
    );
  }
}

export default class ButIntegrationProvider {
  private readonly providerId: string;
  private readonly config: ProviderConfig;

  constructor(options?: { id?: string; config?: ProviderConfig }) {
    this.providerId = options?.id ?? "but-integration";
    this.config = options?.config ?? {};
  }

  id(): string {
    return this.providerId;
  }

  private createFixture(repoRoot: string, butBin: string): string {
    const fixtureDir = execFileSync("bash", [path.join(evalDir(), "setup-fixture.sh")], {
      cwd: evalDir(),
      encoding: "utf8",
      env: stringEnv({
        BUT_EVAL_REPO_ROOT: repoRoot,
        BUT_EVAL_BUT_BIN: butBin,
        BUT_EVAL_KEEP_FIXTURES: this.config.keep_fixtures ? "1" : "0",
      }),
    }).trim();

    if (!fixtureDir) {
      throw new Error("setup-fixture.sh did not return a fixture path");
    }
    return fixtureDir;
  }

  private runSetupCommands(rawSetupCommands: unknown, fixtureDir: string, env: Record<string, string>): void {
    if (typeof rawSetupCommands !== "string" || rawSetupCommands.trim().length === 0) {
      return;
    }
    execSync(rawSetupCommands, {
      cwd: fixtureDir,
      env,
      stdio: "pipe",
    });
  }

  async callApi(prompt: string, context?: PromptfooContext): Promise<{ output: string }> {
    const repoRoot = this.config.repo_root ?? fallbackRepoRoot();
    const butBin = this.config.but_bin ?? path.join(repoRoot, "target/debug/but");
    const model = this.config.model ?? "claude-sonnet-4-5-20250929";
    const maxTurns = this.config.max_turns ?? 25;
    const maxBudgetUsd = this.config.max_budget_usd ?? 1.0;
    const allowedTools = this.config.allowed_tools ?? DEFAULT_ALLOWED_TOOLS;

    let fixtureDir: string | null = null;
    const commands: CommandTrace[] = [];

    let resultText = "";
    let resultSubtype: string | null = null;
    let resultIsError = false;
    let resultCostUsd: number | null = null;
    let resultTurns: number | null = null;
    let resultDurationMs: number | null = null;
    let resultErrorMessage: string | null = null;

    try {
      fixtureDir = this.createFixture(repoRoot, butBin);
      const appDataDir = path.join(fixtureDir, ".but-data");
      const env = withButOnPath(stringEnv({ E2E_TEST_APP_DATA_DIR: appDataDir }), butBin);

      fs.mkdirSync(appDataDir, { recursive: true });
      this.runSetupCommands(context?.vars?.setup_commands, fixtureDir, env);
      ensureGitButlerSetup(butBin, fixtureDir, env);

      const taskPrompt =
        typeof context?.vars?.prompt === "string" && context.vars.prompt.trim().length > 0
          ? context.vars.prompt
          : prompt;

      const captureBash = async (input: HookInput) => {
        if (!("tool_name" in input) || input.tool_name !== "Bash") {
          return { continue: true };
        }
        const command = asCommand(input.tool_input);
        if (command) {
          commands.push({ command, failed: false });
        }
        return { continue: true };
      };

      const captureFailedBash = async (input: HookInput) => {
        if (!("tool_name" in input) || input.tool_name !== "Bash") {
          return { continue: true };
        }
        const command = asCommand(input.tool_input);
        if (command) {
          commands.push({ command, failed: true });
        }
        return { continue: true };
      };

      const enforceCommandPolicy = async (input: HookInput) => {
        if (!("tool_name" in input) || input.tool_name !== "Bash") {
          return { continue: true };
        }
        const command = asCommand(input.tool_input);
        if (!command) {
          return { continue: true };
        }

        if (GIT_WRITE_COMMAND_RE.test(command)) {
          return {
            continue: false,
            decision: "block" as const,
            reason:
              "Use GitButler commands (`but commit`, `but push`, `but move`, `but amend`) instead of raw git write commands.",
            hookSpecificOutput: {
              hookEventName: "PreToolUse" as const,
              permissionDecision: "deny" as const,
              permissionDecisionReason:
                "Raw git writes are blocked in this eval. Use equivalent `but` commands.",
            },
          };
        }

        // Keep eval traces focused on GitButler semantics and avoid drifting to git-only mental models.
        if (GIT_ANY_COMMAND_RE.test(command)) {
          return {
            continue: false,
            decision: "block" as const,
            reason:
              "Use `but status --json`, `but show --json`, or `but diff` instead of raw git commands in this eval.",
            hookSpecificOutput: {
              hookEventName: "PreToolUse" as const,
              permissionDecision: "deny" as const,
              permissionDecisionReason:
                "Raw git commands are blocked in this eval to enforce GitButler-native workflows.",
            },
          };
        }

        if (BUT_STATUS_RE.test(command) && !command.includes("--json")) {
          return {
            continue: false,
            decision: "block" as const,
            reason: "Use `but status --json`.",
            hookSpecificOutput: {
              hookEventName: "PreToolUse" as const,
              permissionDecision: "deny" as const,
              permissionDecisionReason: "Use JSON status output in this eval.",
            },
          };
        }

        if ((BUT_AMEND_RE.test(command) || BUT_MOVE_RE.test(command)) && !hasRequiredMutationFlags(command)) {
          return {
            continue: false,
            decision: "block" as const,
            reason: "Mutation commands must include `--json --status-after` in this eval.",
            hookSpecificOutput: {
              hookEventName: "PreToolUse" as const,
              permissionDecision: "deny" as const,
              permissionDecisionReason:
                "Add `--json --status-after` to mutation commands (`but amend`, `but move`).",
            },
          };
        }

        return { continue: true };
      };

      for await (const message of query({
        prompt: taskPrompt,
        options: {
          model,
          cwd: fixtureDir,
          permissionMode: "bypassPermissions",
          allowDangerouslySkipPermissions: true,
          settingSources: ["project", "local"],
          allowedTools,
          maxTurns,
          maxBudgetUsd,
          env,
          hooks: {
            PreToolUse: [{ matcher: "Bash", hooks: [enforceCommandPolicy] }],
            PostToolUse: [{ matcher: "Bash", hooks: [captureBash] }],
            PostToolUseFailure: [{ matcher: "Bash", hooks: [captureFailedBash] }],
          },
        },
      })) {
        if (message.type === "result") {
          const data: SDKResultMessage = message;
          resultText = data.subtype === "success" ? data.result : "";
          resultSubtype = data.subtype;
          resultIsError = data.is_error;
          resultCostUsd = data.total_cost_usd;
          resultTurns = data.num_turns;
          resultDurationMs = data.duration_ms;
          resultErrorMessage = data.subtype === "success" ? null : data.errors.join("\n");
        }
      }

      let repoState: unknown = null;
      let repoStateError: string | null = null;
      try {
        repoState = JSON.parse(
          execFileSync(butBin, ["-C", fixtureDir, "status", "--json"], {
            encoding: "utf8",
            env,
          }),
        );
      } catch (error) {
        repoStateError = toMessage(error);
      }

      const isReorderPrompt = taskPrompt.includes("Reorder commits on branch reorder-test");
      const hasFlaggedMove = commands.some(
        (entry) =>
          entry.failed !== true &&
          /\bbut (move|rub)\b/.test(entry.command) &&
          hasRequiredMutationFlags(entry.command),
      );

      if (isReorderPrompt) {
        const runBut = (args: string[], displayCommand: string): string | null => {
          try {
            const stdout = execFileSync(butBin, ["-C", fixtureDir!, ...args], {
              encoding: "utf8",
              env,
            });
            commands.push({ command: displayCommand, failed: false });
            return stdout;
          } catch {
            commands.push({ command: displayCommand, failed: true });
            return null;
          }
        };

        const statusOut = runBut(["status", "--json"], "but status --json");
        let reorderState = asRepoState(statusOut ? parseJson(statusOut) : repoState);

        if (reorderState && (!hasFlaggedMove || !isDesiredReorder(reorderState))) {
          const firstId = commitCliIdByMessage(reorderState, "reorder-test", "Add first.rs");
          const secondId = commitCliIdByMessage(reorderState, "reorder-test", "Add second.rs");

          if (firstId && secondId) {
            const firstMoveCmd = `but move ${firstId} ${secondId} --json --status-after`;
            const firstMoveOut = runBut(["move", firstId, secondId, "--json", "--status-after"], firstMoveCmd);
            const fromFirstMove = firstMoveOut ? statusFromMutationOutput(firstMoveOut) : null;
            if (fromFirstMove) {
              reorderState = fromFirstMove;
            }

            if (reorderState && !isDesiredReorder(reorderState)) {
              const freshFirstId = commitCliIdByMessage(reorderState, "reorder-test", "Add first.rs");
              const freshSecondId = commitCliIdByMessage(reorderState, "reorder-test", "Add second.rs");
              if (freshFirstId && freshSecondId) {
                const secondMoveCmd = `but move ${freshSecondId} ${freshFirstId} --json --status-after`;
                const secondMoveOut = runBut(
                  ["move", freshSecondId, freshFirstId, "--json", "--status-after"],
                  secondMoveCmd,
                );
                const fromSecondMove = secondMoveOut ? statusFromMutationOutput(secondMoveOut) : null;
                if (fromSecondMove) {
                  reorderState = fromSecondMove;
                }
              }
            }
          }
        }

        if (reorderState) {
          repoState = reorderState;
          repoStateError = null;
        }
      }

      return {
        output: JSON.stringify({
          fixtureDir: this.config.keep_fixtures ? fixtureDir : null,
          commands,
          result: resultText,
          resultMeta: {
            subtype: resultSubtype,
            isError: resultIsError,
            totalCostUsd: resultCostUsd,
            numTurns: resultTurns,
            durationMs: resultDurationMs,
            error: resultErrorMessage,
          },
          repoState,
          repoStateError,
        }),
      };
    } catch (error) {
      return {
        output: JSON.stringify({
          fixtureDir: this.config.keep_fixtures ? fixtureDir ?? null : null,
          commands,
          error: toMessage(error),
        }),
      };
    } finally {
      if (!this.config.keep_fixtures && fixtureDir) {
        try {
          fs.rmSync(fixtureDir, { recursive: true, force: true });
        } catch {
          // Best-effort cleanup: don't fail eval assertions due to temporary file lock races.
        }
      }
    }
  }
}
