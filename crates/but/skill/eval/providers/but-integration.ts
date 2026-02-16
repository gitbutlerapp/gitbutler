import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

type ProviderConfig = {
  agent?: "claude" | "codex";
  model?: string;
  repo_root?: string;
  but_bin?: string;
  runner?: string;
  runner_bin?: string;
  runner_timeout_ms?: number;
  min_runner_version?: string;
  claude_bin?: string;
  claude_runner?: string;
  codex_bin?: string;
  codex_runner?: string;
  auth_mode?: "auto" | "local" | "api";
  claude_timeout_ms?: number;
  min_claude_version?: string;
  min_codex_version?: string;
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

type ResultMeta = {
  text: string;
  subtype: string | null;
  isError: boolean;
  costUsd: number | null;
  turns: number | null;
  durationMs: number | null;
  error: string | null;
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

const BASH_TOOL_NAME = "Bash";
const DEFAULT_RUNNER_TIMEOUT_MS = 180_000;
const DEFAULT_MIN_CLAUDE_VERSION = "1.0.88";
const DEFAULT_MIN_CODEX_VERSION = "0.99.0";

function parsePositiveInt(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value) && value > 0) {
    return Math.floor(value);
  }
  if (typeof value !== "string") {
    return null;
  }
  const trimmed = value.trim();
  if (!/^\d+$/.test(trimmed)) {
    return null;
  }
  const parsed = Number.parseInt(trimmed, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return null;
  }
  return parsed;
}

function resolveRunnerTimeoutMs(config: ProviderConfig): number {
  const fromEnv = parsePositiveInt(process.env.BUT_EVAL_RUNNER_TIMEOUT_MS);
  if (fromEnv !== null) {
    return fromEnv;
  }
  const fromLegacyEnv = parsePositiveInt(process.env.BUT_EVAL_CLAUDE_TIMEOUT_MS);
  if (fromLegacyEnv !== null) {
    return fromLegacyEnv;
  }
  const fromConfig = parsePositiveInt(config.runner_timeout_ms ?? config.claude_timeout_ms);
  if (fromConfig !== null) {
    return fromConfig;
  }
  return DEFAULT_RUNNER_TIMEOUT_MS;
}

function parseJson(input: string): unknown {
  try {
    return JSON.parse(input);
  } catch {
    return null;
  }
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

function toStdout(error: unknown): string {
  if (!(error instanceof Error)) {
    return "";
  }
  const maybeStdOut = (error as { stdout?: string | Buffer }).stdout;
  if (typeof maybeStdOut === "string") {
    return maybeStdOut;
  }
  if (Buffer.isBuffer(maybeStdOut)) {
    return maybeStdOut.toString("utf8");
  }
  return "";
}

function toStderr(error: unknown): string {
  if (!(error instanceof Error)) {
    return "";
  }
  const maybeStdErr = (error as { stderr?: string | Buffer }).stderr;
  if (typeof maybeStdErr === "string") {
    return maybeStdErr;
  }
  if (Buffer.isBuffer(maybeStdErr)) {
    return maybeStdErr.toString("utf8");
  }
  return "";
}

function wasTimeout(error: unknown): boolean {
  if (!(error instanceof Error)) {
    return false;
  }
  const maybeError = error as NodeJS.ErrnoException & { killed?: boolean; signal?: string | null };
  if (maybeError.code === "ETIMEDOUT") {
    return true;
  }
  if (maybeError.killed === true && maybeError.signal === "SIGTERM") {
    return true;
  }
  return false;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function asString(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

function asNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function asBoolean(value: unknown): boolean | null {
  return typeof value === "boolean" ? value : null;
}

function parseJsonLines(output: string): unknown[] {
  const events: unknown[] = [];
  for (const line of output.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed.startsWith("{") || !trimmed.endsWith("}")) {
      continue;
    }
    const parsed = parseJson(trimmed);
    if (parsed) {
      events.push(parsed);
    }
  }
  return events;
}

function pushCommand(traces: CommandTrace[], command: string, failed: boolean): void {
  const normalized = command.trim();
  if (normalized.length === 0) {
    return;
  }
  const previous = traces[traces.length - 1];
  if (previous && previous.command === normalized && previous.failed === failed) {
    return;
  }
  traces.push({ command: normalized, failed });
}

function collectBashCommands(value: unknown, traces: CommandTrace[], inBash = false, failed = false): void {
  if (Array.isArray(value)) {
    for (const item of value) {
      collectBashCommands(item, traces, inBash, failed);
    }
    return;
  }

  const record = asRecord(value);
  if (!record) {
    return;
  }

  const type = asString(record.type);
  const name = asString(record.name);
  const toolName = asString(record.tool_name);
  const nextInBash =
    inBash ||
    name === BASH_TOOL_NAME ||
    toolName === BASH_TOOL_NAME ||
    (type === "tool_use" && name === BASH_TOOL_NAME);

  const isFailed =
    failed ||
    asBoolean(record.failed) === true ||
    asBoolean(record.is_error) === true ||
    asBoolean(record.success) === false ||
    (!!record.error && record.error !== false);

  const maybeCommand = asString(record.command);
  if (nextInBash && maybeCommand) {
    pushCommand(traces, maybeCommand, isFailed);
  }

  for (const nested of Object.values(record)) {
    collectBashCommands(nested, traces, nextInBash, isFailed);
  }
}

function collectCodexCommand(value: unknown, traces: CommandTrace[]): void {
  const record = asRecord(value);
  if (!record || asString(record.type) !== "item.completed") {
    return;
  }

  const item = asRecord(record.item);
  if (!item || asString(item.type) !== "command_execution") {
    return;
  }

  const command = asString(item.command);
  if (!command) {
    return;
  }

  const exitCode = asNumber(item.exit_code);
  const status = asString(item.status);
  const failed = (exitCode !== null && exitCode !== 0) || status === "failed";
  pushCommand(traces, command, failed);
}

function extractCommandTrace(events: unknown[]): CommandTrace[] {
  const traces: CommandTrace[] = [];
  for (const event of events) {
    collectBashCommands(event, traces);
    collectCodexCommand(event, traces);
  }
  return traces;
}

function textFromContent(value: unknown): string {
  if (!Array.isArray(value)) {
    return "";
  }

  const pieces: string[] = [];
  for (const block of value) {
    const record = asRecord(block);
    if (!record) {
      continue;
    }
    if (record.type === "text" && typeof record.text === "string") {
      pieces.push(record.text);
    }
  }
  return pieces.join("\n").trim();
}

function extractResultMeta(events: unknown[]): ResultMeta {
  let text = "";
  let subtype: string | null = null;
  let isError = false;
  let costUsd: number | null = null;
  let turns: number | null = null;
  let durationMs: number | null = null;
  let error: string | null = null;
  let lastAssistantText = "";
  let codexTurnCount = 0;

  for (const event of events) {
    const record = asRecord(event);
    if (!record) {
      continue;
    }

    const recordType = asString(record.type);

    const messageRecord = asRecord(record.message);
    if (recordType === "assistant" && messageRecord) {
      const assistantText = textFromContent(messageRecord.content);
      if (assistantText.length > 0) {
        lastAssistantText = assistantText;
      }
    }

    const codexItem = asRecord(record.item);
    if (recordType === "item.completed" && codexItem && asString(codexItem.type) === "agent_message") {
      const assistantText = asString(codexItem.text);
      if (assistantText && assistantText.trim().length > 0) {
        lastAssistantText = assistantText.trim();
      }
    }

    if (recordType === "turn.completed") {
      codexTurnCount += 1;
    }

    const looksLikeResult =
      recordType === "result" ||
      "result" in record ||
      "subtype" in record ||
      "num_turns" in record ||
      "duration_ms" in record;

    if (!looksLikeResult) {
      continue;
    }

    const nextText = asString(record.result);
    const nextSubtype = asString(record.subtype);
    const nextIsError = asBoolean(record.is_error);
    const nextError = asString(record.error);

    if (nextText !== null) {
      text = nextText;
    }
    if (nextSubtype !== null) {
      subtype = nextSubtype;
    }
    if (nextIsError !== null) {
      isError = nextIsError;
    }
    if (nextError !== null && nextError.trim().length > 0) {
      error = nextError;
    }

    const nextCost = asNumber(record.total_cost_usd);
    const nextTurns = asNumber(record.num_turns);
    const nextDuration = asNumber(record.duration_ms);

    if (nextCost !== null) {
      costUsd = nextCost;
    }
    if (nextTurns !== null) {
      turns = nextTurns;
    }
    if (nextDuration !== null) {
      durationMs = nextDuration;
    }
  }

  if (turns === null && codexTurnCount > 0) {
    turns = codexTurnCount;
  }

  if (text.length === 0 && lastAssistantText.length > 0) {
    text = lastAssistantText;
  }

  return {
    text,
    subtype,
    isError,
    costUsd,
    turns,
    durationMs,
    error,
  };
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

function resolvePathInEvalDir(candidatePath: string): string {
  if (path.isAbsolute(candidatePath)) {
    return candidatePath;
  }
  return path.resolve(evalDir(), candidatePath);
}

function buildPolicyPrompt(requirePullCheckBeforePull: boolean): string {
  const lines = [
    "Use GitButler commands instead of raw git commands for workflow changes.",
    "Use `but status --json` when checking status.",
    "For mutation commands (`but commit`, `but amend`, `but move`, `but pull` updates), include `--json --status-after`.",
    "For pull checks, use `but pull --check --json`.",
    "Avoid routine `--help` probes before mutations; use the skill's canonical command patterns first.",
  ];
  if (requirePullCheckBeforePull) {
    lines.push("This task explicitly asks for mergeability check before updating; run `but pull --check --json` before `but pull --json --status-after`.");
  }
  return lines.join("\n");
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
    try {
      execFileSync("bash", ["-euo", "pipefail", "-c", rawSetupCommands], {
        cwd: fixtureDir,
        env,
        stdio: "pipe",
      });
    } catch (error) {
      throw new Error(`Failed setup_commands: ${toMessage(error)}`);
    }
  }

  async callApi(prompt: string, context?: PromptfooContext): Promise<{ output: string }> {
    const repoRoot = this.config.repo_root ?? fallbackRepoRoot();
    const butBin = this.config.but_bin ?? path.join(repoRoot, "target/debug/but");
    const agent = (process.env.BUT_EVAL_AGENT ?? this.config.agent ?? "claude") === "codex" ? "codex" : "claude";
    const runnerBin =
      process.env.BUT_EVAL_RUNNER_BIN ??
      (agent === "codex"
        ? process.env.BUT_EVAL_CODEX_BIN ?? this.config.runner_bin ?? this.config.codex_bin ?? "codex"
        : process.env.BUT_EVAL_CLAUDE_BIN ?? this.config.runner_bin ?? this.config.claude_bin ?? "claude");
    const runnerScript = resolvePathInEvalDir(
      process.env.BUT_EVAL_RUNNER ??
        this.config.runner ??
        (agent === "codex"
          ? this.config.codex_runner ?? "providers/codex-local.sh"
          : this.config.claude_runner ?? "providers/claude-local.sh"),
    );
    const authMode = this.config.auth_mode ?? process.env.BUT_EVAL_AUTH_MODE ?? "auto";
    const model =
      process.env.BUT_EVAL_MODEL ??
      this.config.model ??
      (agent === "codex" ? "gpt-5-codex" : "claude-sonnet-4-5-20250929");
    const allowedTools = this.config.allowed_tools ?? DEFAULT_ALLOWED_TOOLS;
    const runnerTimeoutMs = resolveRunnerTimeoutMs(this.config);
    const minRunnerVersion =
      process.env.BUT_EVAL_MIN_RUNNER_VERSION ??
      (agent === "codex"
        ? process.env.BUT_EVAL_MIN_CODEX_VERSION ??
          this.config.min_runner_version ??
          this.config.min_codex_version ??
          DEFAULT_MIN_CODEX_VERSION
        : process.env.BUT_EVAL_MIN_CLAUDE_VERSION ??
          this.config.min_runner_version ??
          this.config.min_claude_version ??
          DEFAULT_MIN_CLAUDE_VERSION);
    const agentLabel = agent === "codex" ? "Codex" : "Claude";

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
      if (!fs.existsSync(runnerScript)) {
        throw new Error(`${agentLabel} runner script not found: ${runnerScript}`);
      }

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
      const requirePullCheckBeforePull =
        /\bcheck\b[\s\S]*\bmerge cleanly\b[\s\S]*\bupdate\b/i.test(taskPrompt) ||
        /\bmerge cleanly\b[\s\S]*\bthen\b[\s\S]*\bupdate\b/i.test(taskPrompt);

      let rawAgentOutput = "";
      let cliRunError: string | null = null;

      try {
        rawAgentOutput = execFileSync("bash", [runnerScript], {
          cwd: fixtureDir,
          encoding: "utf8",
          stdio: ["ignore", "pipe", "pipe"],
          timeout: runnerTimeoutMs,
          env: {
            ...env,
            BUT_EVAL_AGENT: agent,
            BUT_EVAL_RUNNER_BIN: runnerBin,
            BUT_EVAL_CLAUDE_BIN:
              agent === "claude"
                ? runnerBin
                : process.env.BUT_EVAL_CLAUDE_BIN ?? this.config.claude_bin ?? "claude",
            BUT_EVAL_CODEX_BIN:
              agent === "codex" ? runnerBin : process.env.BUT_EVAL_CODEX_BIN ?? this.config.codex_bin ?? "codex",
            BUT_EVAL_MODEL: model,
            BUT_EVAL_AUTH_MODE: authMode,
            BUT_EVAL_PROMPT: taskPrompt,
            BUT_EVAL_ALLOWED_TOOLS: allowedTools.join(","),
            BUT_EVAL_PERMISSION_MODE: "bypassPermissions",
            BUT_EVAL_APPEND_SYSTEM_PROMPT: buildPolicyPrompt(requirePullCheckBeforePull),
            BUT_EVAL_MIN_RUNNER_VERSION: minRunnerVersion,
            BUT_EVAL_MIN_CLAUDE_VERSION:
              agent === "claude"
                ? minRunnerVersion
                : process.env.BUT_EVAL_MIN_CLAUDE_VERSION ?? this.config.min_claude_version ?? DEFAULT_MIN_CLAUDE_VERSION,
            BUT_EVAL_MIN_CODEX_VERSION:
              agent === "codex"
                ? minRunnerVersion
                : process.env.BUT_EVAL_MIN_CODEX_VERSION ?? this.config.min_codex_version ?? DEFAULT_MIN_CODEX_VERSION,
          },
        });
      } catch (error) {
        const stdout = toStdout(error);
        const stderr = toStderr(error);
        rawAgentOutput = `${stdout}${stdout && stderr ? "\n" : ""}${stderr}`;
        if (wasTimeout(error)) {
          cliRunError = `${agentLabel} runner timed out after ${runnerTimeoutMs}ms.`;
        } else {
          cliRunError = toMessage(error);
        }
      }

      const events = parseJsonLines(rawAgentOutput);
      const capturedCommands = extractCommandTrace(events);
      commands.push(...capturedCommands);

      const meta = extractResultMeta(events);
      resultText = meta.text;
      resultSubtype = meta.subtype;
      resultIsError = meta.isError;
      resultCostUsd = meta.costUsd;
      resultTurns = meta.turns;
      resultDurationMs = meta.durationMs;
      resultErrorMessage = meta.error;

      if (cliRunError) {
        resultIsError = true;
        resultSubtype = resultSubtype ?? "error";
        resultErrorMessage = resultErrorMessage ? `${resultErrorMessage}\n${cliRunError}` : cliRunError;
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
