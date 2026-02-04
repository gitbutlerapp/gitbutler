import fs from "node:fs";
import path from "node:path";

type PromptfooEvalFile = {
  results?: {
    results?: PromptfooResultRow[];
  };
};

type PromptfooResultRow = {
  success?: unknown;
  gradingResult?: {
    pass?: unknown;
  };
  response?: {
    output?: unknown;
  };
  testCase?: {
    description?: unknown;
  };
};

type ProviderOutput = {
  taskPrompt?: unknown;
  commands?: Array<{ command?: unknown; failed?: unknown; eventIndex?: unknown }>;
  editOperations?: Array<{ filePath?: unknown; eventIndex?: unknown }>;
  watchedFiles?: Array<{ path?: unknown; changed?: unknown }>;
  coordinationState?: {
    agents?: unknown[];
    claims?: unknown[];
    messages?: unknown[];
    discoveries?: unknown[];
  };
  repoState?: {
    stacks?: Array<{
      branches?: Array<{
        name?: unknown;
      }>;
    }>;
  } | null;
};

type ScenarioStat = {
  description: string;
  runs: number;
  passes: number;
  passRate: number;
};

const EVAL_AGENT_ID = "tier4-eval-agent";

function usage(): never {
  console.error("Usage: node dist/scripts/natural-metrics.js <output.json> [--threshold <0..1>]");
  process.exit(2);
}

function parseArgs(argv: string[]): { inputPath: string; threshold: number } {
  let inputPath = "";
  let threshold = 0.7;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--threshold") {
      const value = argv[i + 1];
      if (!value) {
        usage();
      }
      const parsed = Number.parseFloat(value);
      if (!Number.isFinite(parsed) || parsed < 0 || parsed > 1) {
        usage();
      }
      threshold = parsed;
      i += 1;
      continue;
    }
    if (!arg.startsWith("--") && inputPath.length === 0) {
      inputPath = arg;
      continue;
    }
    usage();
  }

  if (inputPath.length === 0) {
    usage();
  }

  return { inputPath, threshold };
}

function parseJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
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

function normalizeCommand(command: string): string {
  const trimmed = command.trim();
  const shellWrapped = trimmed.match(/^[^ ]+ -lc '([\s\S]*)'$/);
  if (shellWrapped && shellWrapped[1]) {
    return shellWrapped[1].trim();
  }
  return trimmed;
}

function parseProviderOutput(row: PromptfooResultRow): ProviderOutput {
  const rawOutput = asString(row.response?.output);
  if (!rawOutput) {
    return {};
  }

  const parsed = parseJson(rawOutput);
  const record = asRecord(parsed);
  if (!record) {
    return {};
  }

  return record as unknown as ProviderOutput;
}

function descriptionOf(row: PromptfooResultRow): string {
  const desc = asString(row.testCase?.description);
  return desc && desc.length > 0 ? desc : "(unknown)";
}

function rowPassed(row: PromptfooResultRow): boolean {
  if (row.success === true) {
    return true;
  }
  if (row.gradingResult?.pass === true) {
    return true;
  }
  return false;
}

function successfulCommandEntries(output: ProviderOutput): Array<{ command: string; eventIndex: number | null }> {
  const commands = Array.isArray(output.commands) ? output.commands : [];
  return commands
    .filter((entry) => entry?.failed !== true)
    .map((entry) => ({
      command: typeof entry?.command === "string" ? normalizeCommand(entry.command) : "",
      eventIndex: typeof entry?.eventIndex === "number" ? entry.eventIndex : null,
    }))
    .filter((entry) => entry.command.length > 0);
}

function watchedPaths(output: ProviderOutput): string[] {
  if (!Array.isArray(output.watchedFiles)) {
    return [];
  }
  return output.watchedFiles
    .map((file) => (typeof file?.path === "string" ? file.path : ""))
    .filter((pathValue) => pathValue.length > 0);
}

function watchedFileChanged(output: ProviderOutput, watchedPath: string): boolean | null {
  if (!Array.isArray(output.watchedFiles)) {
    return null;
  }
  const match = output.watchedFiles.find((file) => file?.path === watchedPath);
  if (!match) {
    return null;
  }
  return match.changed === true;
}

function promptHasExplicitCheck(output: ProviderOutput, watchedPath: string): boolean {
  const prompt = asString(output.taskPrompt);
  if (!prompt) {
    return false;
  }
  const lower = prompt.toLowerCase();
  return lower.includes("but-engineering check") && lower.includes(watchedPath.toLowerCase());
}

function hasCheckCommand(output: ProviderOutput, watchedPath: string): boolean {
  return successfulCommandEntries(output).some(
    (entry) => /\bbut-engineering\s+check\b/.test(entry.command) && entry.command.includes(watchedPath),
  );
}

function hasStackCheckCommand(output: ProviderOutput, watchedPath: string): boolean {
  return successfulCommandEntries(output).some(
    (entry) =>
      /\bbut-engineering\s+check\b/.test(entry.command) &&
      entry.command.includes(watchedPath) &&
      /--include-stack(\s|$)/.test(entry.command),
  );
}

function hasButStatusJson(output: ProviderOutput): boolean {
  return successfulCommandEntries(output).some(
    (entry) => /\bbut\s+status\b/.test(entry.command) && /\s--json(\s|$)/.test(entry.command),
  );
}

function hasDependencyCoordinationMessage(output: ProviderOutput): boolean {
  const messages = output.coordinationState?.messages;
  if (!Array.isArray(messages)) {
    return false;
  }
  return messages.some((entry) => {
    const record = asRecord(entry);
    if (!record || record.agent_id !== EVAL_AGENT_ID || typeof record.content !== "string") {
      return false;
    }
    const content = record.content.toLowerCase();
    return content.includes("auth-base") || content.includes("profile-ui") || content.includes("dependency");
  });
}

function hasDoneAnnouncementFromEvalAgent(output: ProviderOutput): boolean {
  const messages = output.coordinationState?.messages;
  if (!Array.isArray(messages)) {
    return false;
  }
  return messages.some((entry) => {
    const record = asRecord(entry);
    if (!record || record.agent_id !== EVAL_AGENT_ID || typeof record.content !== "string") {
      return false;
    }
    return /^DONE:/i.test(record.content.trim());
  });
}

function hasStackAnchorOrAlignment(output: ProviderOutput): boolean {
  const commandAlignment = successfulCommandEntries(output).some((entry) => {
    const cmd = entry.command;
    if (/\bbut\s+branch\s+new\b/.test(cmd) && /(?:\s-a|\s--anchor)\s+\S+/.test(cmd)) {
      return true;
    }
    if (/\bbut\s+commit\b/.test(cmd) && cmd.includes("--json") && cmd.includes("--status-after")) {
      return true;
    }
    return false;
  });

  if (commandAlignment) {
    return true;
  }

  const stacks = Array.isArray(output.repoState?.stacks) ? output.repoState?.stacks ?? [] : [];
  return stacks.some((stack) => Array.isArray(stack?.branches) && stack.branches.length >= 2);
}

function firstPlanIndex(output: ProviderOutput): number | null {
  for (const entry of successfulCommandEntries(output)) {
    if (/\bbut-engineering\s+plan\b/.test(entry.command)) {
      return entry.eventIndex;
    }
  }
  return null;
}

function firstEditIndex(output: ProviderOutput, watchedPath: string): number | null {
  const editOps = Array.isArray(output.editOperations) ? output.editOperations : [];
  let earliest: number | null = null;

  for (const op of editOps) {
    if (op?.filePath !== watchedPath || typeof op?.eventIndex !== "number") {
      continue;
    }
    if (earliest === null || op.eventIndex < earliest) {
      earliest = op.eventIndex;
    }
  }

  for (const entry of successfulCommandEntries(output)) {
    if (!entry.command.includes(watchedPath) || entry.eventIndex === null) {
      continue;
    }
    if (/\bbut-engineering\s+/.test(entry.command)) {
      continue;
    }
    if (!/(?:>|>>|tee\b|sed\s+-i|perl\s+-pi|cat\s+<<|apply_patch|mv\b|cp\b)/.test(entry.command)) {
      continue;
    }
    if (earliest === null || entry.eventIndex < earliest) {
      earliest = entry.eventIndex;
    }
  }

  return earliest;
}

function planBeforeEdit(output: ProviderOutput, watchedPath: string): boolean {
  const planIndex = firstPlanIndex(output);
  if (planIndex === null) {
    return false;
  }

  const editIndex = firstEditIndex(output, watchedPath);
  if (editIndex === null) {
    const changed = watchedFileChanged(output, watchedPath);
    return changed === true;
  }

  return planIndex < editIndex;
}

function hasDiscoveryFromEvalAgent(output: ProviderOutput): boolean {
  const discoveries = output.coordinationState?.discoveries;
  if (!Array.isArray(discoveries)) {
    return false;
  }
  return discoveries.some((entry) => {
    const record = asRecord(entry);
    return record?.agent_id === EVAL_AGENT_ID;
  });
}

function hasReleaseAllCommand(output: ProviderOutput): boolean {
  return successfulCommandEntries(output).some(
    (entry) => /\bbut-engineering\s+release\b/.test(entry.command) && /--all(\s|$)/.test(entry.command),
  );
}

function hasPlanClearCommand(output: ProviderOutput): boolean {
  return successfulCommandEntries(output).some(
    (entry) => /\bbut-engineering\s+plan\b/.test(entry.command) && /--clear(\s|$)/.test(entry.command),
  );
}

function hasRemainingEvalAgentClaims(output: ProviderOutput): boolean {
  const claims = Array.isArray(output.coordinationState?.claims) ? output.coordinationState?.claims : [];
  return claims.some((entry) => {
    const record = asRecord(entry);
    return record?.agent_id === EVAL_AGENT_ID;
  });
}

function hasExclusiveEvalAgentClaim(output: ProviderOutput, watchedPath: string): boolean {
  const claims = Array.isArray(output.coordinationState?.claims) ? output.coordinationState?.claims : [];
  const owners = new Set<string>();
  for (const entry of claims) {
    const record = asRecord(entry);
    if (!record || record.file_path !== watchedPath || typeof record.agent_id !== "string") {
      continue;
    }
    owners.add(record.agent_id);
  }
  return owners.size === 1 && owners.has(EVAL_AGENT_ID);
}

function hasRedundantSyncAfterCheckBeforeEdit(output: ProviderOutput, watchedPath: string): boolean {
  const entries = successfulCommandEntries(output);
  const checkEntry = entries.find(
    (entry) => /\bbut-engineering\s+check\b/.test(entry.command) && entry.command.includes(watchedPath),
  );
  const checkIndex = checkEntry?.eventIndex ?? null;
  if (checkIndex === null) {
    return false;
  }

  const editIndex = firstEditIndex(output, watchedPath);
  if (editIndex === null || editIndex <= checkIndex) {
    return false;
  }

  const syncCommands = entries
    .filter((entry) => entry.eventIndex !== null && entry.eventIndex > checkIndex && entry.eventIndex < editIndex)
    .map((entry) => entry.command)
    .filter((command) => /\bbut-engineering\s+(read|post|check|plan|agents|claims|status)\b/.test(command));

  const extraChecks = syncCommands.filter((command) => /\bbut-engineering\s+check\b/.test(command)).length;
  if (extraChecks > 0) {
    return true;
  }

  return syncCommands.length > 1;
}

function evalAgentPlanState(output: ProviderOutput): "set" | "cleared" | "missing" {
  const agents = Array.isArray(output.coordinationState?.agents) ? output.coordinationState?.agents : [];
  const selfRecord = agents.find((entry) => {
    const record = asRecord(entry);
    return record?.id === EVAL_AGENT_ID || record?.agent_id === EVAL_AGENT_ID;
  });
  if (!selfRecord) {
    return "missing";
  }
  const record = asRecord(selfRecord);
  const planValue = record?.plan;
  if (planValue === null || planValue === undefined) {
    return "cleared";
  }
  if (typeof planValue === "string" && planValue.trim().length === 0) {
    return "cleared";
  }
  return "set";
}

function compositeCooperationSuccess(output: ProviderOutput): boolean {
  const baseChanged = watchedFileChanged(output, "src/auth/base.rs");
  const profileChanged = watchedFileChanged(output, "src/profile/handler.rs");
  const parserChanged = watchedFileChanged(output, "src/parser.rs");

  const hasStatus = hasButStatusJson(output);
  const hasStackCheck = !promptHasExplicitCheck(output, "src/profile/handler.rs") && hasStackCheckCommand(output, "src/profile/handler.rs");
  const hasCoordination = hasDependencyCoordinationMessage(output);
  const hasStackAction = hasStackAnchorOrAlignment(output);

  const planOrderOk = planBeforeEdit(output, "src/profile/handler.rs") && planBeforeEdit(output, "src/parser.rs");
  const hasDiscovery = hasDiscoveryFromEvalAgent(output);
  const hasReleaseAll = hasReleaseAllCommand(output);
  const noClaims = !hasRemainingEvalAgentClaims(output);

  const planState = evalAgentPlanState(output);
  const planClearObserved = hasPlanClearCommand(output);
  const planClean = planState !== "set" && (planClearObserved || planState === "cleared");

  return (
    baseChanged === false &&
    profileChanged === true &&
    parserChanged === true &&
    hasStatus &&
    hasStackCheck &&
    hasCoordination &&
    hasStackAction &&
    planOrderOk &&
    hasDiscovery &&
    hasReleaseAll &&
    noClaims &&
    planClean
  );
}

function pct(num: number, den: number): number {
  if (den <= 0) {
    return 0;
  }
  return num / den;
}

function main(): void {
  const { inputPath, threshold } = parseArgs(process.argv.slice(2));
  const resolved = path.resolve(inputPath);

  if (!fs.existsSync(resolved)) {
    console.error(`Output file not found: ${resolved}`);
    process.exit(2);
  }

  const parsed = parseJson(fs.readFileSync(resolved, "utf8"));
  const evalFile = (asRecord(parsed) ?? {}) as unknown as PromptfooEvalFile;
  const rows = Array.isArray(evalFile.results?.results) ? evalFile.results?.results ?? [] : [];

  if (rows.length === 0) {
    console.error("No result rows found in promptfoo output.");
    process.exit(2);
  }

  const scenarioMap = new Map<string, { runs: number; passes: number }>();

  let totalPasses = 0;
  let autonomousCheckOpportunities = 0;
  let autonomousCheckSuccess = 0;
  let planBeforeEditOpportunities = 0;
  let planBeforeEditSuccess = 0;
  let discoveryGotchaOpportunities = 0;
  let discoveryGotchaSuccess = 0;
  let contestedOverwriteOpportunities = 0;
  let contestedOverwriteCount = 0;
  let stackDependencyOpportunities = 0;
  let stackDependencySuccess = 0;
  let compositeCoordinationOpportunities = 0;
  let compositeCoordinationSuccess = 0;
  let redundantSyncOwnerOpportunities = 0;
  let redundantSyncOwnerSuccess = 0;
  let doneAnnouncementOpportunities = 0;
  let doneAnnouncementSuccess = 0;

  for (const row of rows) {
    const description = descriptionOf(row);
    const passed = rowPassed(row);
    const stat = scenarioMap.get(description) ?? { runs: 0, passes: 0 };
    stat.runs += 1;
    if (passed) {
      stat.passes += 1;
      totalPasses += 1;
    }
    scenarioMap.set(description, stat);

    const output = parseProviderOutput(row);
    const watched = watchedPaths(output);
    const primaryWatched = watched[0] ?? "";

    if (/conflict|advisory/i.test(description) && primaryWatched.length > 0) {
      autonomousCheckOpportunities += 1;
      if (!promptHasExplicitCheck(output, primaryWatched) && hasCheckCommand(output, primaryWatched)) {
        autonomousCheckSuccess += 1;
      }
    }

    if (/plan|gotcha/i.test(description) && primaryWatched.length > 0) {
      planBeforeEditOpportunities += 1;
      if (planBeforeEdit(output, primaryWatched)) {
        planBeforeEditSuccess += 1;
      }

      discoveryGotchaOpportunities += 1;
      if (hasDiscoveryFromEvalAgent(output)) {
        discoveryGotchaSuccess += 1;
      }
    }

    if (/conflict/i.test(description) && primaryWatched.length > 0) {
      contestedOverwriteOpportunities += 1;
      if (watchedFileChanged(output, primaryWatched) === true) {
        contestedOverwriteCount += 1;
      }
    }

    if (/dependency|stack dependency/i.test(description) && primaryWatched.length > 0) {
      stackDependencyOpportunities += 1;
      const stackCheck = !promptHasExplicitCheck(output, primaryWatched) && hasStackCheckCommand(output, primaryWatched);
      const hasStackContextSignal = hasButStatusJson(output) || stackCheck;
      if (
        hasStackContextSignal &&
        stackCheck &&
        hasDependencyCoordinationMessage(output) &&
        hasStackAnchorOrAlignment(output)
      ) {
        stackDependencySuccess += 1;
      }
    }

    if (/composite/i.test(description)) {
      compositeCoordinationOpportunities += 1;
      if (compositeCooperationSuccess(output)) {
        compositeCoordinationSuccess += 1;
      }
    }

    if (/plan|discover|composite|completion/i.test(description)) {
      doneAnnouncementOpportunities += 1;
      if (hasDoneAnnouncementFromEvalAgent(output)) {
        doneAnnouncementSuccess += 1;
      }
    }

    if (primaryWatched.length > 0 && hasCheckCommand(output, primaryWatched) && hasExclusiveEvalAgentClaim(output, primaryWatched)) {
      redundantSyncOwnerOpportunities += 1;
      if (!hasRedundantSyncAfterCheckBeforeEdit(output, primaryWatched)) {
        redundantSyncOwnerSuccess += 1;
      }
    }
  }

  const totalRuns = rows.length;
  const overallPassRate = pct(totalPasses, totalRuns);

  const scenarios: ScenarioStat[] = Array.from(scenarioMap.entries())
    .map(([description, stat]) => ({
      description,
      runs: stat.runs,
      passes: stat.passes,
      passRate: pct(stat.passes, stat.runs),
    }))
    .sort((a, b) => a.description.localeCompare(b.description));

  const report = {
    suite: path.basename(resolved).includes("composite") ? "composite" : "natural",
    threshold,
    overall: {
      runs: totalRuns,
      passes: totalPasses,
      passRate: overallPassRate,
      meetsThreshold: overallPassRate >= threshold,
    },
    scenarios,
    kpis: {
      autonomous_check_rate: {
        opportunities: autonomousCheckOpportunities,
        successes: autonomousCheckSuccess,
        rate: pct(autonomousCheckSuccess, autonomousCheckOpportunities),
      },
      plan_before_edit_rate: {
        opportunities: planBeforeEditOpportunities,
        successes: planBeforeEditSuccess,
        rate: pct(planBeforeEditSuccess, planBeforeEditOpportunities),
      },
      discovery_on_gotcha_rate: {
        opportunities: discoveryGotchaOpportunities,
        successes: discoveryGotchaSuccess,
        rate: pct(discoveryGotchaSuccess, discoveryGotchaOpportunities),
      },
      contested_file_overwrite_rate: {
        opportunities: contestedOverwriteOpportunities,
        overwrites: contestedOverwriteCount,
        rate: pct(contestedOverwriteCount, contestedOverwriteOpportunities),
      },
      stack_dependency_coordination_rate: {
        opportunities: stackDependencyOpportunities,
        successes: stackDependencySuccess,
        rate: pct(stackDependencySuccess, stackDependencyOpportunities),
      },
      composite_coordination_rate: {
        opportunities: compositeCoordinationOpportunities,
        successes: compositeCoordinationSuccess,
        rate: pct(compositeCoordinationSuccess, compositeCoordinationOpportunities),
      },
      redundant_sync_while_owner_rate: {
        opportunities: redundantSyncOwnerOpportunities,
        successes: redundantSyncOwnerSuccess,
        rate: pct(redundantSyncOwnerSuccess, redundantSyncOwnerOpportunities),
      },
      done_announcement_rate: {
        opportunities: doneAnnouncementOpportunities,
        successes: doneAnnouncementSuccess,
        rate: pct(doneAnnouncementSuccess, doneAnnouncementOpportunities),
      },
    },
  };

  console.log(JSON.stringify(report, null, 2));

  if (overallPassRate < threshold) {
    process.exit(1);
  }
}

main();
