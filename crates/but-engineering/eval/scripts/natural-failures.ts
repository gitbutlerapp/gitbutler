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
    blocks?: unknown[];
  };
  repoState?: {
    stacks?: Array<{
      branches?: Array<{
        name?: unknown;
      }>;
    }>;
  } | null;
};

type ParsedArgs = {
  inputPath: string;
  scenarioFilter: string | null;
  json: boolean;
};

type ScenarioType = "conflict" | "advisory" | "plan_discover" | "stack_dependency" | "composite" | "unknown";

type FailureDiagnostic = {
  rowIndex: number;
  description: string;
  scenario: ScenarioType;
  missingSignals: string[];
  watchedFiles: Array<{ path: string; changed: boolean | null }>;
  evidenceCommands: string[];
};

const EVAL_AGENT_ID = "tier4-eval-agent";

function usage(): never {
  console.error(
    "Usage: node dist/scripts/natural-failures.js <output.json> [--scenario <substring>] [--json]",
  );
  process.exit(2);
}

function parseArgs(argv: string[]): ParsedArgs {
  let inputPath = "";
  let scenarioFilter: string | null = null;
  let json = false;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--json") {
      json = true;
      continue;
    }
    if (arg === "--scenario") {
      const value = argv[i + 1];
      if (!value) {
        usage();
      }
      scenarioFilter = value.toLowerCase();
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

  return { inputPath, scenarioFilter, json };
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

function descriptionOf(row: PromptfooResultRow): string {
  const value = asString(row.testCase?.description);
  return value && value.length > 0 ? value : "(unknown)";
}

function rowPassed(row: PromptfooResultRow): boolean {
  return row.success === true || row.gradingResult?.pass === true;
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

function watchedFiles(output: ProviderOutput): Array<{ path: string; changed: boolean | null }> {
  const watched = Array.isArray(output.watchedFiles) ? output.watchedFiles : [];
  return watched
    .map((entry) => ({
      path: typeof entry?.path === "string" ? entry.path : "",
      changed: entry?.changed === true ? true : entry?.changed === false ? false : null,
    }))
    .filter((entry) => entry.path.length > 0);
}

function watchedFileChanged(output: ProviderOutput, watchedPath: string): boolean | null {
  return watchedFiles(output).find((file) => file.path === watchedPath)?.changed ?? null;
}

function primaryWatchedPath(output: ProviderOutput): string | null {
  return watchedFiles(output)[0]?.path ?? null;
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

function promptText(output: ProviderOutput): string {
  return asString(output.taskPrompt) ?? "";
}

function hasCheckCommandForFile(output: ProviderOutput, watchedPath: string): boolean {
  return successfulCommandEntries(output).some(
    (entry) => /\bbut-engineering\s+check\b/.test(entry.command) && entry.command.includes(watchedPath),
  );
}

function hasStackCheckCommandForFile(output: ProviderOutput, watchedPath: string): boolean {
  return successfulCommandEntries(output).some(
    (entry) =>
      /\bbut-engineering\s+check\b/.test(entry.command) &&
      entry.command.includes(watchedPath) &&
      /--include-stack(\s|$)/.test(entry.command),
  );
}

function promptExplicitlyRequestsCheck(output: ProviderOutput, watchedPath: string): boolean {
  const prompt = promptText(output).toLowerCase();
  return prompt.includes("but-engineering check") && prompt.includes(watchedPath.toLowerCase());
}

function usedAutonomousCheck(output: ProviderOutput, watchedPath: string): boolean {
  return hasCheckCommandForFile(output, watchedPath) && !promptExplicitlyRequestsCheck(output, watchedPath);
}

function usedAutonomousStackCheck(output: ProviderOutput, watchedPath: string): boolean {
  return hasStackCheckCommandForFile(output, watchedPath) && !promptExplicitlyRequestsCheck(output, watchedPath);
}

function hasButStatusJson(output: ProviderOutput): boolean {
  return successfulCommandEntries(output).some(
    (entry) => /\bbut\s+status\b/.test(entry.command) && /\s--json(\s|$)/.test(entry.command),
  );
}

function hasCoordinationAction(output: ProviderOutput): boolean {
  return successfulCommandEntries(output).some((entry) =>
    /\bbut-engineering\s+(read|post|check|plan)\b/.test(entry.command),
  );
}

function hasPlanCommand(output: ProviderOutput): boolean {
  return successfulCommandEntries(output).some((entry) => /\bbut-engineering\s+plan\b/.test(entry.command));
}

function hasDiscoverCommand(output: ProviderOutput): boolean {
  return successfulCommandEntries(output).some((entry) => /\bbut-engineering\s+discover\b/.test(entry.command));
}

function hasReleaseCommand(output: ProviderOutput): boolean {
  return successfulCommandEntries(output).some((entry) => /\bbut-engineering\s+release\b/.test(entry.command));
}

function hasPlanClearCommand(output: ProviderOutput): boolean {
  return successfulCommandEntries(output).some(
    (entry) => /\bbut-engineering\s+plan\b/.test(entry.command) && /--clear(\s|$)/.test(entry.command),
  );
}

function hasDiscoveryMessageFromEvalAgent(output: ProviderOutput): boolean {
  const discoveries = Array.isArray(output.coordinationState?.discoveries) ? output.coordinationState?.discoveries : [];
  return discoveries.some((entry) => {
    const record = asRecord(entry);
    return record?.agent_id === EVAL_AGENT_ID;
  });
}

function hasRemainingEvalAgentClaims(output: ProviderOutput): boolean {
  const claims = Array.isArray(output.coordinationState?.claims) ? output.coordinationState?.claims : [];
  return claims.some((entry) => {
    const record = asRecord(entry);
    return record?.agent_id === EVAL_AGENT_ID;
  });
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

function hasBlockForFile(output: ProviderOutput, watchedPath: string): boolean {
  const blocks = Array.isArray(output.coordinationState?.blocks) ? output.coordinationState?.blocks : [];
  return blocks.some((entry) => {
    const record = asRecord(entry);
    return typeof record?.content === "string" && record.content.includes(watchedPath);
  });
}

function hasDependencyCoordinationMessage(output: ProviderOutput): boolean {
  const messages = Array.isArray(output.coordinationState?.messages) ? output.coordinationState?.messages : [];
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
  const messages = Array.isArray(output.coordinationState?.messages) ? output.coordinationState?.messages : [];
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

function wroteContestedBaseFile(output: ProviderOutput): boolean {
  return successfulCommandEntries(output).some((entry) => commandMutatesPath(entry.command, "src/auth/base.rs"));
}

function firstCommandEventIndex(output: ProviderOutput, predicate: (command: string) => boolean): number | null {
  return (
    successfulCommandEntries(output).find((entry) => entry.eventIndex !== null && predicate(entry.command))?.eventIndex ??
    null
  );
}

function commandMutatesPath(command: string, relativePath: string): boolean {
  if (!command.includes(relativePath)) {
    return false;
  }
  if (/\bbut-engineering\s+/.test(command)) {
    return false;
  }
  return (
    /(?:^|\s)(?:apply_patch|sed\s+-i|perl\s+-pi|tee\b|mv\b|cp\b)\b/.test(command) ||
    /cat\s+<<[\s\S]*>/.test(command) ||
    /(?:>|>>)/.test(command)
  );
}

function firstMutationCommandEventIndex(output: ProviderOutput, relativePath: string): number | null {
  return firstCommandEventIndex(output, (command) => commandMutatesPath(command, relativePath));
}

function firstEditOperationEventIndex(output: ProviderOutput, relativePath: string): number | null {
  const editOps = Array.isArray(output.editOperations) ? output.editOperations : [];
  let minIndex: number | null = null;
  for (const op of editOps) {
    if (op?.filePath !== relativePath || typeof op?.eventIndex !== "number") {
      continue;
    }
    if (minIndex === null || op.eventIndex < minIndex) {
      minIndex = op.eventIndex;
    }
  }
  return minIndex;
}

function planBeforeEdit(output: ProviderOutput, watchedPath: string): boolean {
  const planIndex = firstCommandEventIndex(output, (command) => /\bbut-engineering\s+plan\b/.test(command));
  if (planIndex === null) {
    return false;
  }

  const editToolIndex = firstEditOperationEventIndex(output, watchedPath);
  const mutationIndex = firstMutationCommandEventIndex(output, watchedPath);
  const firstEditIndex =
    editToolIndex === null
      ? mutationIndex
      : mutationIndex === null
        ? editToolIndex
        : Math.min(editToolIndex, mutationIndex);

  if (firstEditIndex === null) {
    return watchedFileChanged(output, watchedPath) === true;
  }

  return planIndex < firstEditIndex;
}

function classifyScenario(description: string): ScenarioType {
  const lower = description.toLowerCase();
  if (lower.includes("composite")) {
    return "composite";
  }
  if (lower.includes("conflict")) {
    return "conflict";
  }
  if (lower.includes("dependency") || (lower.includes("stack") && lower.includes("coordinate"))) {
    return "stack_dependency";
  }
  if (lower.includes("advisory")) {
    return "advisory";
  }
  if (lower.includes("plan") || lower.includes("gotcha") || lower.includes("discover")) {
    return "plan_discover";
  }
  return "unknown";
}

function diagnoseFailure(row: PromptfooResultRow, rowIndex: number): FailureDiagnostic {
  const description = descriptionOf(row);
  const scenario = classifyScenario(description);
  const output = parseProviderOutput(row);
  const watched = watchedFiles(output);
  const primaryPath = watched[0]?.path ?? null;
  const missingSignals: string[] = [];

  if (scenario === "conflict") {
    if (primaryPath && !usedAutonomousCheck(output, primaryPath)) {
      missingSignals.push("missing_autonomous_check");
    }
    if (primaryPath && watchedFileChanged(output, primaryPath) === true) {
      missingSignals.push("contested_file_changed");
    }
    if (!hasCoordinationAction(output)) {
      missingSignals.push("missing_coordination_action");
    }
  } else if (scenario === "plan_discover") {
    if (!hasPlanCommand(output)) {
      missingSignals.push("missing_plan_command");
    }
    if (primaryPath && !planBeforeEdit(output, primaryPath)) {
      missingSignals.push("plan_not_before_edit");
    }
    if (!hasDiscoverCommand(output) && !hasDiscoveryMessageFromEvalAgent(output)) {
      missingSignals.push("missing_discover_command_or_message");
    }
    if (!hasReleaseCommand(output)) {
      missingSignals.push("missing_release_command");
    }
    if (hasRemainingEvalAgentClaims(output)) {
      missingSignals.push("claims_not_cleaned");
    }
    if (!hasDoneAnnouncementFromEvalAgent(output)) {
      missingSignals.push("missing_done_announcement");
    }
  } else if (scenario === "advisory") {
    if (primaryPath && !usedAutonomousCheck(output, primaryPath)) {
      missingSignals.push("missing_autonomous_check");
    }
    if (primaryPath && watchedFileChanged(output, primaryPath) !== true) {
      missingSignals.push("file_not_changed_when_allowed");
    }
    if (primaryPath && hasBlockForFile(output, primaryPath)) {
      missingSignals.push("unexpected_block_for_file");
    }
  } else if (scenario === "stack_dependency") {
    const hasStackCheck = primaryPath ? usedAutonomousStackCheck(output, primaryPath) : false;
    const hasStackContextSignal = hasButStatusJson(output) || hasStackCheck;
    if (!hasStackContextSignal) {
      missingSignals.push("missing_status_check");
    }
    if (primaryPath && !hasStackCheck) {
      missingSignals.push("missing_stack_dependency_check");
    }
    if (!hasDependencyCoordinationMessage(output)) {
      missingSignals.push("missing_dependency_coordination_message");
    }
    if (!hasStackAnchorOrAlignment(output)) {
      missingSignals.push("missing_stack_anchor_or_alignment");
    }
    if (wroteContestedBaseFile(output)) {
      missingSignals.push("base_file_overwritten");
    }
  } else if (scenario === "composite") {
    const profileChanged = watchedFileChanged(output, "src/profile/handler.rs") === true;
    const parserChanged = watchedFileChanged(output, "src/parser.rs") === true;
    if (!profileChanged || !parserChanged) {
      missingSignals.push("missing_dual_delivery_change");
    }

    if (watchedFileChanged(output, "src/auth/base.rs") === true || wroteContestedBaseFile(output)) {
      missingSignals.push("base_file_overwritten");
    }

    const hasStackCheck = usedAutonomousStackCheck(output, "src/profile/handler.rs");
    if (!hasButStatusJson(output)) {
      missingSignals.push("missing_status_check");
    }
    if (!hasStackCheck) {
      missingSignals.push("missing_stack_dependency_check");
    }
    if (!hasDependencyCoordinationMessage(output)) {
      missingSignals.push("missing_dependency_coordination_message");
    }
    if (!hasStackAnchorOrAlignment(output)) {
      missingSignals.push("missing_stack_anchor_or_alignment");
    }

    if (!hasPlanCommand(output)) {
      missingSignals.push("missing_plan_command");
    }
    if (!planBeforeEdit(output, "src/profile/handler.rs") || !planBeforeEdit(output, "src/parser.rs")) {
      missingSignals.push("plan_not_before_edit");
    }
    if (!hasDiscoverCommand(output) && !hasDiscoveryMessageFromEvalAgent(output)) {
      missingSignals.push("missing_discover_command_or_message");
    }
    if (!hasReleaseCommand(output)) {
      missingSignals.push("missing_release_command");
    }
    if (hasRemainingEvalAgentClaims(output)) {
      missingSignals.push("claims_not_cleaned");
    }

    const planState = evalAgentPlanState(output);
    const planClearObserved = hasPlanClearCommand(output);
    if (planState === "set" || (!planClearObserved && planState !== "cleared")) {
      missingSignals.push("missing_plan_clear_command_or_state");
    }
    if (!hasDoneAnnouncementFromEvalAgent(output)) {
      missingSignals.push("missing_done_announcement");
    }
  }

  if (missingSignals.length === 0) {
    missingSignals.push("unknown_failure_signal");
  }

  return {
    rowIndex,
    description,
    scenario,
    missingSignals,
    watchedFiles: watched,
    evidenceCommands: successfulCommandEntries(output)
      .slice(0, 10)
      .map((entry) => entry.command),
  };
}

function printTextReport(
  diagnostics: FailureDiagnostic[],
  totalRows: number,
  failedRows: number,
  scenarioFilter: string | null,
): void {
  console.log(`rows_total=${totalRows} failed_rows=${failedRows} matched_failures=${diagnostics.length}`);
  if (scenarioFilter) {
    console.log(`scenario_filter=${scenarioFilter}`);
  }

  if (diagnostics.length === 0) {
    console.log("No failed rows matched filter.");
    return;
  }

  diagnostics.forEach((diag, idx) => {
    console.log("");
    console.log(`[${idx + 1}] ${diag.description}`);
    console.log(`scenario: ${diag.scenario}`);
    console.log(`missing_signals: ${diag.missingSignals.join(", ")}`);
    if (diag.watchedFiles.length > 0) {
      const watchedSummary = diag.watchedFiles.map((w) => `${w.path}=${String(w.changed)}`).join(", ");
      console.log(`watched_files: ${watchedSummary}`);
    } else {
      console.log("watched_files: (none)");
    }
    console.log("evidence_commands:");
    if (diag.evidenceCommands.length === 0) {
      console.log("  (none)");
    } else {
      for (const command of diag.evidenceCommands) {
        console.log(`  - ${command}`);
      }
    }
  });
}

function main(): void {
  const args = parseArgs(process.argv.slice(2));
  const resolved = path.resolve(args.inputPath);

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

  const failed = rows
    .map((row, index) => ({ row, index }))
    .filter(({ row }) => !rowPassed(row))
    .filter(({ row }) => {
      if (!args.scenarioFilter) {
        return true;
      }
      return descriptionOf(row).toLowerCase().includes(args.scenarioFilter);
    });
  const failedRows = rows.filter((row) => !rowPassed(row)).length;

  const diagnostics = failed.map(({ row, index }) => diagnoseFailure(row, index));

  if (args.json) {
    console.log(
      JSON.stringify(
        {
          input: resolved,
          scenarioFilter: args.scenarioFilter,
          totalRows: rows.length,
          failedRows,
          matchedFailures: diagnostics.length,
          diagnostics,
        },
        null,
        2,
      ),
    );
  } else {
    printTextReport(diagnostics, rows.length, failedRows, args.scenarioFilter);
  }
}

main();
