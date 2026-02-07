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
