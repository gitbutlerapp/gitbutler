type CommandTrace = {
  command?: unknown;
  failed?: unknown;
  eventIndex?: unknown;
};

type EditOperation = {
  tool?: unknown;
  filePath?: unknown;
  eventIndex?: unknown;
};

type WatchedFile = {
  path?: unknown;
  changed?: unknown;
};

type CoordinationState = {
  agents?: unknown[];
  claims?: unknown[];
  messages?: unknown[];
  discoveries?: unknown[];
  blocks?: unknown[];
};

type EvalOutput = {
  commands?: CommandTrace[];
  editOperations?: EditOperation[];
  watchedFiles?: WatchedFile[];
  coordinationState?: CoordinationState;
  repoState?: {
    stacks?: Array<{
      branches?: Array<{
        name?: unknown;
      }>;
    }>;
  } | null;
  taskPrompt?: unknown;
  error?: unknown;
  resultMeta?: {
    isError?: unknown;
  };
};

const EVAL_AGENT_ID = "tier4-eval-agent";

function parseOutput(output: unknown): EvalOutput {
  if (typeof output !== "string") {
    return {};
  }
  try {
    return JSON.parse(output) as EvalOutput;
  } catch {
    return {};
  }
}

function normalizeCommand(command: string): string {
  const trimmed = command.trim();
  const shellWrapped = trimmed.match(/^[^ ]+ -lc '([\s\S]*)'$/);
  if (shellWrapped && shellWrapped[1]) {
    return shellWrapped[1].trim();
  }
  return trimmed;
}

function successfulCommands(data: EvalOutput): string[] {
  return commandEntries(data)
    .filter((entry) => !entry.failed)
    .map((entry) => entry.command)
    .filter((command) => command.length > 0);
}

function commandEntries(data: EvalOutput): Array<{ command: string; failed: boolean; eventIndex: number | null }> {
  return (data.commands || []).map((entry) => {
    const command = typeof entry?.command === "string" ? normalizeCommand(entry.command) : "";
    const failed = entry?.failed === true;
    const eventIndex = typeof entry?.eventIndex === "number" ? entry.eventIndex : null;
    return { command, failed, eventIndex };
  });
}

function watchedFileChanged(data: EvalOutput, relativePath: string): boolean | null {
  if (!Array.isArray(data.watchedFiles)) {
    return null;
  }
  const match = data.watchedFiles.find((entry) => entry?.path === relativePath);
  if (!match) {
    return null;
  }
  return match.changed === true;
}

function messageContent(entry: unknown): string {
  if (!entry || typeof entry !== "object") {
    return "";
  }
  const value = (entry as { content?: unknown }).content;
  return typeof value === "string" ? value : "";
}

function messageAgentId(entry: unknown): string {
  if (!entry || typeof entry !== "object") {
    return "";
  }
  const value = (entry as { agent_id?: unknown }).agent_id;
  return typeof value === "string" ? value : "";
}

function claimFilePath(entry: unknown): string {
  if (!entry || typeof entry !== "object") {
    return "";
  }
  const value = (entry as { file_path?: unknown }).file_path;
  return typeof value === "string" ? value : "";
}

function hasFatalError(data: EvalOutput): boolean {
  return !!data.error || data.resultMeta?.isError === true;
}

function promptText(data: EvalOutput): string {
  return typeof data.taskPrompt === "string" ? data.taskPrompt : "";
}

function hasCheckCommandForFile(data: EvalOutput, filePath: string): boolean {
  return successfulCommands(data).some((cmd) => {
    if (!/\bbut-engineering\s+check\b/.test(cmd)) {
      return false;
    }
    return cmd.includes(filePath);
  });
}

function hasStackDependencyCheckForFile(data: EvalOutput, filePath: string): boolean {
  return successfulCommands(data).some((cmd) => {
    if (!/\bbut-engineering\s+check\b/.test(cmd)) {
      return false;
    }
    if (!cmd.includes(filePath)) {
      return false;
    }
    return /--include-stack(\s|$)/.test(cmd);
  });
}

function hasButStatusJson(data: EvalOutput): boolean {
  return successfulCommands(data).some((cmd) => /\bbut\s+status\b/.test(cmd) && /\s--json(\s|$)/.test(cmd));
}

function promptExplicitlyRequestsCheck(data: EvalOutput, filePath: string): boolean {
  const prompt = promptText(data).toLowerCase();
  if (!prompt.includes("but-engineering check")) {
    return false;
  }
  return prompt.includes(filePath.toLowerCase());
}

function usedAutonomousCheck(data: EvalOutput, filePath: string): boolean {
  return hasCheckCommandForFile(data, filePath) && !promptExplicitlyRequestsCheck(data, filePath);
}

function usedAutonomousStackCheck(data: EvalOutput, filePath: string): boolean {
  return hasStackDependencyCheckForFile(data, filePath) && !promptExplicitlyRequestsCheck(data, filePath);
}

function firstCommandEventIndex(data: EvalOutput, predicate: (command: string) => boolean): number | null {
  const index = commandEntries(data)
    .filter((entry) => !entry.failed)
    .find((entry) => entry.eventIndex !== null && predicate(entry.command));
  return index?.eventIndex ?? null;
}

function commandMutatesPath(command: string, relativePath: string): boolean {
  if (!command.includes(relativePath)) {
    return false;
  }

  // Exclude coordination commands that mention file paths without editing.
  if (/\bbut-engineering\s+(check|post|read|plan|discover|claim|release|claims|agents|status)\b/.test(command)) {
    return false;
  }

  return (
    /(?:^|\s)(?:apply_patch|sed\s+-i|perl\s+-pi|tee\b|mv\b|cp\b)\b/.test(command) ||
    /cat\s+<<[\s\S]*>/.test(command) ||
    /(?:>|>>)/.test(command)
  );
}

function firstMutationCommandEventIndex(data: EvalOutput, relativePath: string): number | null {
  return firstCommandEventIndex(data, (command) => commandMutatesPath(command, relativePath));
}

function firstEditOperationEventIndex(data: EvalOutput, relativePath: string): number | null {
  if (!Array.isArray(data.editOperations)) {
    return null;
  }

  let minIndex: number | null = null;
  for (const op of data.editOperations) {
    if (op?.filePath !== relativePath) {
      continue;
    }
    if (typeof op?.eventIndex !== "number") {
      continue;
    }
    if (minIndex === null || op.eventIndex < minIndex) {
      minIndex = op.eventIndex;
    }
  }
  return minIndex;
}

function firstObservedEditEventIndex(data: EvalOutput, relativePath: string): number | null {
  const editToolIndex = firstEditOperationEventIndex(data, relativePath);
  const commandMutationIndex = firstMutationCommandEventIndex(data, relativePath);

  if (editToolIndex === null) {
    return commandMutationIndex;
  }
  if (commandMutationIndex === null) {
    return editToolIndex;
  }
  return Math.min(editToolIndex, commandMutationIndex);
}

function planBeforeEdit(data: EvalOutput, relativePath: string): boolean {
  const planIndex = firstCommandEventIndex(data, (command) => /\bbut-engineering\s+plan\b/.test(command));
  const editIndex = firstObservedEditEventIndex(data, relativePath);

  if (planIndex === null) {
    return false;
  }

  // If no edit marker was observed but the file changed, we can at least require
  // that plan exists (event traces can differ by runner/tooling).
  if (editIndex === null) {
    const changed = watchedFileChanged(data, relativePath);
    return changed === true;
  }

  return planIndex < editIndex;
}

function commandBeforeEdit(
  data: EvalOutput,
  relativePath: string,
  predicate: (command: string) => boolean,
): boolean {
  const commandIndex = firstCommandEventIndex(data, predicate);
  const editIndex = firstObservedEditEventIndex(data, relativePath);
  if (commandIndex === null) {
    return false;
  }
  if (editIndex === null) {
    const changed = watchedFileChanged(data, relativePath);
    return changed === true;
  }
  return commandIndex < editIndex;
}

function announceListenBeforeEdit(data: EvalOutput, relativePath: string): boolean {
  const postIndex = firstCommandEventIndex(data, (command) => /\bbut-engineering\s+post\b/.test(command));
  const readIndex = firstCommandEventIndex(data, (command) => /\bbut-engineering\s+read\b/.test(command));
  const editIndex = firstObservedEditEventIndex(data, relativePath);

  if (postIndex === null || readIndex === null) {
    return false;
  }

  if (editIndex === null) {
    const changed = watchedFileChanged(data, relativePath);
    return changed === true && postIndex <= readIndex;
  }

  return postIndex <= readIndex && postIndex < editIndex && readIndex < editIndex;
}

function repoBranchNames(data: EvalOutput): string[] {
  const stacks = Array.isArray(data.repoState?.stacks) ? data.repoState?.stacks ?? [] : [];
  const names: string[] = [];
  for (const stack of stacks) {
    const branches = Array.isArray(stack?.branches) ? stack.branches : [];
    for (const branch of branches) {
      if (typeof branch?.name === "string" && branch.name.length > 0) {
        names.push(branch.name);
      }
    }
  }
  return names;
}

function hasPlanClearCommand(data: EvalOutput): boolean {
  return successfulCommands(data).some(
    (cmd) => /\bbut-engineering\s+plan\b/.test(cmd) && /--clear(\s|$)/.test(cmd),
  );
}

function hasReleaseAllCommand(data: EvalOutput): boolean {
  return successfulCommands(data).some(
    (cmd) => /\bbut-engineering\s+release\b/.test(cmd) && /--all(\s|$)/.test(cmd),
  );
}

function evalAgentPlanState(data: EvalOutput): "set" | "cleared" | "missing" {
  const agents = Array.isArray(data.coordinationState?.agents) ? data.coordinationState?.agents ?? [] : [];
  const selfRecord = agents.find((entry) => {
    if (!entry || typeof entry !== "object") {
      return false;
    }
    const idValue = (entry as { id?: unknown; agent_id?: unknown }).id;
    const legacyIdValue = (entry as { id?: unknown; agent_id?: unknown }).agent_id;
    return idValue === EVAL_AGENT_ID || legacyIdValue === EVAL_AGENT_ID;
  });

  if (!selfRecord || typeof selfRecord !== "object") {
    return "missing";
  }

  const planValue = (selfRecord as { plan?: unknown }).plan;
  if (planValue === null || planValue === undefined) {
    return "cleared";
  }
  if (typeof planValue === "string" && planValue.trim().length === 0) {
    return "cleared";
  }
  return "set";
}

function hasCoordinationMessageForBranches(data: EvalOutput, branches: string[]): boolean {
  const messages = Array.isArray(data.coordinationState?.messages) ? data.coordinationState?.messages ?? [] : [];
  return messages.some((msg) => {
    if (messageAgentId(msg) !== EVAL_AGENT_ID) {
      return false;
    }
    const content = messageContent(msg).toLowerCase();
    return branches.some((branch) => content.includes(branch.toLowerCase()));
  });
}

function hasDoneAnnouncementFromEvalAgent(data: EvalOutput): boolean {
  const messages = Array.isArray(data.coordinationState?.messages) ? data.coordinationState?.messages ?? [] : [];
  return messages.some((entry) => {
    if (messageAgentId(entry) !== EVAL_AGENT_ID) {
      return false;
    }
    return /^DONE:/i.test(messageContent(entry).trim());
  });
}

function hasStackAnchorOrCommit(data: EvalOutput): boolean {
  return successfulCommands(data).some((cmd) => {
    if (/\bbut\s+branch\s+new\b/.test(cmd) && /(?:\s-a|\s--anchor)\s+\S+/.test(cmd)) {
      return true;
    }
    if (/\bbut\s+commit\b/.test(cmd) && cmd.includes("--json") && cmd.includes("--status-after")) {
      return true;
    }
    return false;
  });
}

function hasRedundantSyncAfterCheckBeforeEdit(data: EvalOutput, relativePath: string): boolean {
  const checkIndex = firstCommandEventIndex(
    data,
    (command) => /\bbut-engineering\s+check\b/.test(command) && command.includes(relativePath),
  );
  const editIndex = firstObservedEditEventIndex(data, relativePath);
  if (checkIndex === null || editIndex === null || editIndex <= checkIndex) {
    return false;
  }

  const syncCommands = commandEntries(data)
    .filter((entry) => !entry.failed && entry.eventIndex !== null && entry.eventIndex > checkIndex && entry.eventIndex < editIndex)
    .map((entry) => entry.command)
    .filter((command) => /\bbut-engineering\s+(read|post|check|plan|agents|claims|status)\b/.test(command));

  const extraChecks = syncCommands.filter((command) => /\bbut-engineering\s+check\b/.test(command)).length;
  if (extraChecks > 0) {
    return true;
  }

  // Allow one lightweight sync command after check, but flag loops/churn.
  return syncCommands.length > 1;
}

export function conflictBlockFlow(output: unknown): boolean {
  const data = parseOutput(output);
  if (hasFatalError(data)) {
    return false;
  }

  const commands = successfulCommands(data);
  const changed = watchedFileChanged(data, "src/auth/login.rs");
  const coordinationAction = commands.some((cmd) => /\bbut-engineering\s+(read|post|check|plan)\b/.test(cmd));

  const allMessages = Array.isArray(data.coordinationState?.messages) ? data.coordinationState?.messages ?? [] : [];
  const blocks = Array.isArray(data.coordinationState?.blocks) ? data.coordinationState?.blocks ?? [] : [];
  const hasFileReference = [...allMessages, ...blocks].some((msg) => messageContent(msg).includes("src/auth/login.rs"));

  return changed === false && coordinationAction && hasFileReference;
}

export function singleBlockPostPerWindow(output: unknown): boolean {
  const data = parseOutput(output);
  if (hasFatalError(data)) {
    return false;
  }

  const blocks = Array.isArray(data.coordinationState?.blocks) ? data.coordinationState?.blocks ?? [] : [];
  const blockedFile = "src/auth/login.rs";
  const matching = blocks.filter(
    (entry) => messageAgentId(entry) === EVAL_AGENT_ID && messageContent(entry).includes(blockedFile),
  );

  return matching.length <= 1;
}

export function advisoryCheckFlow(output: unknown): boolean {
  const data = parseOutput(output);
  if (hasFatalError(data)) {
    return false;
  }

  const commands = successfulCommands(data);
  const ranCheck = commands.some((cmd) => /\bbut-engineering\s+check\s+src\/config\.rs\b/.test(cmd));
  const changed = watchedFileChanged(data, "src/config.rs");

  const blocks = Array.isArray(data.coordinationState?.blocks) ? data.coordinationState?.blocks ?? [] : [];
  const hasBlockForFile = blocks.some((msg) => messageContent(msg).includes("src/config.rs"));

  return ranCheck && changed === true && !hasBlockForFile;
}

export function planDiscoverFlow(output: unknown): boolean {
  const data = parseOutput(output);
  if (hasFatalError(data)) {
    return false;
  }

  const commands = successfulCommands(data);
  const hasPlanCommand = commands.some((cmd) => /\bbut-engineering\s+plan\b/.test(cmd));
  const hasDiscoverCommand = commands.some((cmd) => /\bbut-engineering\s+discover\b/.test(cmd));
  const hasReleaseAllCommand = commands.some((cmd) =>
    /\bbut-engineering\s+release\b/.test(cmd) && /\s--all(\s|$)/.test(cmd),
  );

  const discoveries = Array.isArray(data.coordinationState?.discoveries)
    ? data.coordinationState?.discoveries ?? []
    : [];
  const hasAgentDiscovery = discoveries.some((entry) => messageAgentId(entry) === EVAL_AGENT_ID);

  const claims = Array.isArray(data.coordinationState?.claims) ? data.coordinationState?.claims ?? [] : [];
  const hasRemainingAgentClaim = claims.some((entry) => messageAgentId(entry) === EVAL_AGENT_ID);

  return hasPlanCommand && hasDiscoverCommand && hasReleaseAllCommand && hasAgentDiscovery && !hasRemainingAgentClaim;
}

export function completionAnnouncementFlow(output: unknown): boolean {
  const data = parseOutput(output);
  if (hasFatalError(data)) {
    return false;
  }

  const changed = watchedFileChanged(data, "README.md");
  const commands = successfulCommands(data);
  const hasDoneCommand = commands.some((cmd) => /\bbut-engineering\s+done\b/.test(cmd));
  const hasDoneAnnouncement = hasDoneAnnouncementFromEvalAgent(data);

  const claims = Array.isArray(data.coordinationState?.claims) ? data.coordinationState?.claims ?? [] : [];
  const hasRemainingAgentClaim = claims.some((entry) => messageAgentId(entry) === EVAL_AGENT_ID);
  const planState = evalAgentPlanState(data);

  return changed === true && hasDoneCommand && hasDoneAnnouncement && !hasRemainingAgentClaim && planState !== "set";
}

// -----------------------------------------------------------------------------
// Natural-behavior eval assertions
// -----------------------------------------------------------------------------

export function naturalConflictAutonomyFlow(output: unknown): boolean {
  const data = parseOutput(output);
  if (hasFatalError(data)) {
    return false;
  }

  const changed = watchedFileChanged(data, "src/auth/login.rs");
  const autonomousCheck = usedAutonomousCheck(data, "src/auth/login.rs");
  const coordinationAction = successfulCommands(data).some((cmd) =>
    /\bbut-engineering\s+(read|post|check|plan)\b/.test(cmd),
  );

  const allMessages = Array.isArray(data.coordinationState?.messages) ? data.coordinationState?.messages ?? [] : [];
  const blocks = Array.isArray(data.coordinationState?.blocks) ? data.coordinationState?.blocks ?? [] : [];
  const hasFileReference = [...allMessages, ...blocks].some((msg) => messageContent(msg).includes("src/auth/login.rs"));

  const claims = Array.isArray(data.coordinationState?.claims) ? data.coordinationState?.claims ?? [] : [];
  const peerClaimStillActive = claims.some(
    (claim) => claimFilePath(claim) === "src/auth/login.rs" && messageAgentId(claim) === "peer-a",
  );

  const noContestedOverwrite = changed === false || !peerClaimStillActive;

  return autonomousCheck && coordinationAction && hasFileReference && noContestedOverwrite && changed === false;
}

export function naturalPlanDiscoverDisciplineFlow(output: unknown): boolean {
  const data = parseOutput(output);
  if (hasFatalError(data)) {
    return false;
  }

  const changed = watchedFileChanged(data, "src/parser.rs");
  const obeyedPlanOrder = planBeforeEdit(data, "src/parser.rs");

  const commands = successfulCommands(data);
  const hasPlanCommand = commands.some((cmd) => /\bbut-engineering\s+plan\b/.test(cmd));
  const hasDiscoverCommand = commands.some((cmd) => /\bbut-engineering\s+discover\b/.test(cmd));
  const hasAnyReleaseCommand = commands.some((cmd) => /\bbut-engineering\s+release\b/.test(cmd));

  const discoveries = Array.isArray(data.coordinationState?.discoveries)
    ? data.coordinationState?.discoveries ?? []
    : [];
  const hasAgentDiscovery = discoveries.some((entry) => messageAgentId(entry) === EVAL_AGENT_ID);

  const claims = Array.isArray(data.coordinationState?.claims) ? data.coordinationState?.claims ?? [] : [];
  const hasRemainingAgentClaim = claims.some((entry) => messageAgentId(entry) === EVAL_AGENT_ID);

  return (
    changed === true &&
    hasPlanCommand &&
    obeyedPlanOrder &&
    hasDiscoverCommand &&
    hasAgentDiscovery &&
    hasAnyReleaseCommand &&
    !hasRemainingAgentClaim
  );
}

export function naturalAdvisoryAutonomyFlow(output: unknown): boolean {
  const data = parseOutput(output);
  if (hasFatalError(data)) {
    return false;
  }

  const autonomousCheck = usedAutonomousCheck(data, "src/config.rs");
  const changed = watchedFileChanged(data, "src/config.rs");

  const blocks = Array.isArray(data.coordinationState?.blocks) ? data.coordinationState?.blocks ?? [] : [];
  const hasBlockForFile = blocks.some((msg) => messageContent(msg).includes("src/config.rs"));

  return autonomousCheck && changed === true && !hasBlockForFile;
}

export function naturalAnnounceListenBeforeEditFlow(output: unknown): boolean {
  const data = parseOutput(output);
  if (hasFatalError(data)) {
    return false;
  }

  const changed = watchedFileChanged(data, "README.md");
  const autonomousCheck = usedAutonomousCheck(data, "README.md");
  const plannedBeforeEdit = planBeforeEdit(data, "README.md");
  const coordinatedBeforeEdit = announceListenBeforeEdit(data, "README.md");
  const checkBeforeEdit = commandBeforeEdit(data, "README.md", (command) =>
    /\bbut-engineering\s+check\s+README\.md\b/.test(command),
  );

  return changed === true && autonomousCheck && plannedBeforeEdit && coordinatedBeforeEdit && checkBeforeEdit;
}

export function noRedundantSyncWhenExclusiveOwner(output: unknown): boolean {
  const data = parseOutput(output);
  if (hasFatalError(data)) {
    return false;
  }

  const changed = watchedFileChanged(data, "README.md");
  const checkBeforeEdit = commandBeforeEdit(data, "README.md", (command) =>
    /\bbut-engineering\s+check\s+README\.md\b/.test(command),
  );
  const redundantSync = hasRedundantSyncAfterCheckBeforeEdit(data, "README.md");

  return changed === true && checkBeforeEdit && !redundantSync;
}

export function stackDependencyCoordinationFlow(output: unknown): boolean {
  const data = parseOutput(output);
  if (hasFatalError(data)) {
    return false;
  }

  const changed = watchedFileChanged(data, "src/auth/base.rs");
  const hasStatus = hasButStatusJson(data);
  const hasStackCheck = hasStackDependencyCheckForFile(data, "src/auth/base.rs");
  const coordinated = hasCoordinationMessageForBranches(data, ["auth-base", "profile-ui"]);
  const hasStackAction = hasStackAnchorOrCommit(data) || repoBranchNames(data).includes("profile-ui");

  return changed === false && hasStatus && hasStackCheck && coordinated && hasStackAction;
}

export function stackCommitLockRecoveryFlow(output: unknown): boolean {
  const data = parseOutput(output);
  if (hasFatalError(data)) {
    return false;
  }

  const baseChanged = watchedFileChanged(data, "src/auth/base.rs");
  const profileChanged = watchedFileChanged(data, "src/profile/handler.rs");
  const hasStatus = hasButStatusJson(data);
  const hasStackCheck = hasStackDependencyCheckForFile(data, "src/profile/handler.rs");
  const coordinated =
    hasCoordinationMessageForBranches(data, ["auth-base", "profile-ui"]) ||
    successfulCommands(data).some((cmd) => /\bbut-engineering\s+post\b/.test(cmd) && /lock|dependency/i.test(cmd));
  const hasStackAction = hasStackAnchorOrCommit(data) || repoBranchNames(data).includes("profile-ui");

  return baseChanged === false && profileChanged === true && hasStatus && hasStackCheck && coordinated && hasStackAction;
}

export function naturalStackDependencyAutonomyFlow(output: unknown): boolean {
  const data = parseOutput(output);
  if (hasFatalError(data)) {
    return false;
  }

  const changed = watchedFileChanged(data, "src/profile/handler.rs");
  const hasStatus = hasButStatusJson(data);
  const hasStackCheck = usedAutonomousStackCheck(data, "src/profile/handler.rs");
  const hasStackContextSignal = hasStatus || hasStackCheck;
  const coordinated = hasCoordinationMessageForBranches(data, ["auth-base", "profile-ui"]);
  const hasStackAction = hasStackAnchorOrCommit(data) || repoBranchNames(data).includes("profile-ui");

  return changed === true && hasStackContextSignal && hasStackCheck && coordinated && hasStackAction;
}

export function naturalCompositeCooperationFlow(output: unknown): boolean {
  const data = parseOutput(output);
  if (hasFatalError(data)) {
    return false;
  }

  const baseChanged = watchedFileChanged(data, "src/auth/base.rs");
  const profileChanged = watchedFileChanged(data, "src/profile/handler.rs");
  const parserChanged = watchedFileChanged(data, "src/parser.rs");

  const hasStatus = hasButStatusJson(data);
  const hasStackCheck = usedAutonomousStackCheck(data, "src/profile/handler.rs");
  const coordinated = hasCoordinationMessageForBranches(data, ["auth-base", "profile-ui"]);
  const hasStackAction = hasStackAnchorOrCommit(data) || repoBranchNames(data).includes("profile-ui");

  const planBeforeProfileEdit = planBeforeEdit(data, "src/profile/handler.rs");
  const planBeforeParserEdit = planBeforeEdit(data, "src/parser.rs");

  const discoveries = Array.isArray(data.coordinationState?.discoveries)
    ? data.coordinationState?.discoveries ?? []
    : [];
  const hasAgentDiscovery = discoveries.some((entry) => messageAgentId(entry) === EVAL_AGENT_ID);

  const claims = Array.isArray(data.coordinationState?.claims) ? data.coordinationState?.claims ?? [] : [];
  const hasRemainingAgentClaim = claims.some((entry) => messageAgentId(entry) === EVAL_AGENT_ID);
  const hasReleaseAll = hasReleaseAllCommand(data);

  const planState = evalAgentPlanState(data);
  const hasPlanClear = hasPlanClearCommand(data);
  const planClean = planState !== "set" && (hasPlanClear || planState === "cleared");

  return (
    baseChanged === false &&
    profileChanged === true &&
    parserChanged === true &&
    hasStatus &&
    hasStackCheck &&
    coordinated &&
    hasStackAction &&
    planBeforeProfileEdit &&
    planBeforeParserEdit &&
    hasAgentDiscovery &&
    hasReleaseAll &&
    !hasRemainingAgentClaim &&
    planClean
  );
}
