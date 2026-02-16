type CommandTrace = {
  command?: unknown;
  failed?: unknown;
};

type RepoChange = {
  filePath?: unknown;
};

type RepoCommit = {
  message?: unknown;
};

type RepoBranch = {
  cliId?: unknown;
  name?: unknown;
  commits?: RepoCommit[];
};

type RepoStack = {
  branches?: RepoBranch[];
};

type EvalOutput = {
  commands?: CommandTrace[];
  result?: unknown;
  resultMeta?: {
    isError?: unknown;
  };
  repoState?: {
    unassignedChanges?: RepoChange[];
    stacks?: RepoStack[];
  } | null;
  repoStateError?: unknown;
};

const GIT_WRITE_RE = /\bgit (add|commit|push|merge|rebase|checkout)\b/;
const GIT_WRITE_NO_REBASE_RE = /\bgit (add|commit|push|merge|checkout)\b/;

function normalizeCommand(command: string): string {
  const trimmed = command.trim();
  const shellWrapped = trimmed.match(/^[^ ]+ -lc '([\s\S]*)'$/);
  if (shellWrapped && shellWrapped[1]) {
    return shellWrapped[1].trim();
  }
  return trimmed;
}

function isHelpCommand(command: string): boolean {
  return /\s--help(\s|$)/.test(` ${normalizeCommand(command)} `);
}

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

function commandStrings(data: EvalOutput): string[] {
  return (data.commands || [])
    .filter((entry) => entry?.failed !== true)
    .map((entry) =>
      typeof entry?.command === "string" ? normalizeCommand(entry.command) : "",
    )
    .filter((command) => command.length > 0);
}

function attemptedCommandStrings(data: EvalOutput): string[] {
  return (data.commands || [])
    .map((entry) =>
      typeof entry?.command === "string" ? normalizeCommand(entry.command) : "",
    )
    .filter((command) => command.length > 0);
}

function hasTargetedButCommit(commands: string[]): boolean {
  const commitCmds = commands.filter(
    (cmd) => cmd.includes("but commit") && !isHelpCommand(cmd),
  );
  return commitCmds.some(
    (cmd) =>
      cmd.includes("--changes") &&
      cmd.includes("--json") &&
      cmd.includes("--status-after"),
  );
}

function hasTargetedButMutation(
  commands: string[],
  matcher: RegExp,
): boolean {
  const mutationCmds = commands.filter(
    (cmd) => matcher.test(cmd) && !isHelpCommand(cmd),
  );
  return mutationCmds.some(
    (cmd) => cmd.includes("--json") && cmd.includes("--status-after"),
  );
}

function hasRepoState(data: EvalOutput): boolean {
  return !!data.repoState && !data.repoStateError;
}

function unassignedChanges(data: EvalOutput): RepoChange[] {
  return Array.isArray(data.repoState?.unassignedChanges)
    ? data.repoState!.unassignedChanges!
    : [];
}

function branches(data: EvalOutput): RepoBranch[] {
  if (!Array.isArray(data.repoState?.stacks)) {
    return [];
  }
  return data.repoState!.stacks!.flatMap((stack) =>
    Array.isArray(stack?.branches) ? stack.branches : [],
  );
}

function containsGitWrite(commands: string[]): boolean {
  return commands.some((cmd) => GIT_WRITE_RE.test(cmd));
}

function containsGitWriteNoRebase(commands: string[]): boolean {
  return commands.some((cmd) => GIT_WRITE_NO_REBASE_RE.test(cmd));
}

function outputText(data: EvalOutput): string {
  return typeof data.result === "string" ? data.result : "";
}

function isErrorResult(data: EvalOutput): boolean {
  return data.resultMeta?.isError === true;
}

export function basicCommitFlow(output: unknown): boolean {
  const data = parseOutput(output);
  const commands = commandStrings(data);

  const statusIndex = commands.findIndex((cmd) => cmd.includes("but status"));
  const commitIndex = commands.findIndex(
    (cmd) => cmd.includes("but commit") && !isHelpCommand(cmd),
  );
  const commitCmd = commitIndex >= 0 ? commands[commitIndex] : "";
  const hasChanges = commitCmd.includes("--changes");
  const hasJson = commitCmd.includes("--json");
  const hasStatusAfter = commitCmd.includes("--status-after");
  const redundantStatusAfterCommit =
    commitIndex >= 0 &&
    commands.slice(commitIndex + 1).some((cmd) => cmd.includes("but status"));

  return (
    statusIndex >= 0 &&
    commitIndex > statusIndex &&
    hasChanges &&
    hasJson &&
    hasStatusAfter &&
    !redundantStatusAfterCommit &&
    !containsGitWrite(commands)
  );
}

export function branchWorkflow(output: unknown): boolean {
  const data = parseOutput(output);
  const commands = commandStrings(data);

  const branchIndex = commands.findIndex((cmd) =>
    /\bbut branch new dark-mode\b/.test(cmd),
  );
  const commitIndex = commands.findIndex(
    (cmd) => cmd.includes("but commit") && !isHelpCommand(cmd),
  );
  const commitCmds = commands.filter(
    (cmd) => cmd.includes("but commit") && !isHelpCommand(cmd),
  );
  const createdWithCommitFlag = commitCmds.some(
    (cmd) => /\bbut commit dark-mode\b/.test(cmd) && /\s-c(\s|$)/.test(cmd),
  );
  const hasBranchCreation = branchIndex >= 0 || createdWithCommitFlag;
  const orderingOk = branchIndex >= 0 ? commitIndex > branchIndex : commitIndex >= 0;

  return (
    hasBranchCreation &&
    orderingOk &&
    hasTargetedButCommit(commands) &&
    !containsGitWrite(commands)
  );
}

export function gitSynonymRedirect(output: unknown): boolean {
  const data = parseOutput(output);
  const commands = commandStrings(data);
  const butPush = commands.some((cmd) => /\bbut push\b/.test(cmd));
  const gitPush = commands.some((cmd) => /\bgit push\b/.test(cmd));
  return butPush && !gitPush;
}

export function orderingFlow(output: unknown): boolean {
  const data = parseOutput(output);
  const commands = commandStrings(data);
  const statusIndex = commands.findIndex((cmd) => cmd.includes("but status"));
  const commitIndex = commands.findIndex(
    (cmd) => cmd.includes("but commit") && !isHelpCommand(cmd),
  );
  return (
    statusIndex >= 0 &&
    commitIndex > statusIndex &&
    hasTargetedButCommit(commands) &&
    !containsGitWrite(commands)
  );
}

export function specificityFlow(output: unknown): boolean {
  const data = parseOutput(output);
  const commands = commandStrings(data);
  const unassigned = unassignedChanges(data);
  const bStillUnassigned = unassigned.some((c) => c.filePath === "src/b.rs");
  const aNotUnassigned = !unassigned.some((c) => c.filePath === "src/a.rs");

  return (
    hasTargetedButCommit(commands) &&
    !containsGitWrite(commands) &&
    hasRepoState(data) &&
    bStillUnassigned &&
    aNotUnassigned
  );
}

export function amendFlow(output: unknown): boolean {
  const data = parseOutput(output);
  const commands = commandStrings(data);
  const statusIndex = commands.findIndex((cmd) => cmd.includes("but status"));
  const amendIndex = commands.findIndex(
    (cmd) => cmd.includes("but amend") && !isHelpCommand(cmd),
  );
  const amendCmd = amendIndex >= 0 ? commands[amendIndex] : "";
  const hasJson = amendCmd.includes("--json");
  const hasStatusAfter = amendCmd.includes("--status-after");
  const unassigned = unassignedChanges(data);
  const aNotUnassigned = !unassigned.some((c) => c.filePath === "src/a.rs");

  return (
    statusIndex >= 0 &&
    amendIndex > statusIndex &&
    hasJson &&
    hasStatusAfter &&
    !containsGitWrite(commands) &&
    hasRepoState(data) &&
    aNotUnassigned
  );
}

export function reorderFlow(output: unknown): boolean {
  const data = parseOutput(output);
  const commands = commandStrings(data);
  const statusIndex = commands.findIndex((cmd) => cmd.includes("but status"));
  const reorderIndex = commands.findIndex((cmd) => /\bbut (move|rub)\b/.test(cmd));
  const hasFlaggedReorder = hasTargetedButMutation(commands, /\bbut (move|rub)\b/);
  const noGitRebase = !commands.some((cmd) => /\bgit rebase\b/.test(cmd));
  const reorderBranch = branches(data).find((branch) => branch?.name === "reorder-test");
  const commitMessages = Array.isArray(reorderBranch?.commits)
    ? reorderBranch!.commits!.map((commit) =>
        typeof commit?.message === "string" ? commit.message : "",
      )
    : [];
  const firstIndex = commitMessages.findIndex((message) =>
    message.includes("Add first.rs"),
  );
  const secondIndex = commitMessages.findIndex((message) =>
    message.includes("Add second.rs"),
  );
  const reordered = firstIndex >= 0 && secondIndex >= 0 && firstIndex < secondIndex;

  return (
    statusIndex >= 0 &&
    reorderIndex > statusIndex &&
    hasFlaggedReorder &&
    noGitRebase &&
    !containsGitWriteNoRebase(commands) &&
    hasRepoState(data) &&
    reordered
  );
}

export function conflictResolveFlow(output: unknown): boolean {
  const data = parseOutput(output);
  const commands = commandStrings(data);
  const attempted = attemptedCommandStrings(data);
  const enterIndex = attempted.findIndex(
    (cmd) =>
      /\bbut resolve\b/.test(cmd) &&
      !/\bbut resolve (status|finish|cancel)\b/.test(cmd),
  );
  const statusIndex = attempted.findIndex(
    (cmd, idx) => idx > enterIndex && /\bbut resolve status\b/.test(cmd),
  );
  const finishIndex = attempted.findIndex(
    (cmd, idx) => idx > statusIndex && /\bbut resolve finish\b/.test(cmd),
  );

  const enterCmd = enterIndex >= 0 ? attempted[enterIndex] : "";
  const finishCmd = finishIndex >= 0 ? attempted[finishIndex] : "";
  const enterHasFlags = enterCmd.includes("--json") && enterCmd.includes("--status-after");
  const finishHasFlags = finishCmd.includes("--json") && finishCmd.includes("--status-after");
  const finishOrderingOk = finishIndex < 0 || finishIndex > statusIndex;
  const enterShapeOk = enterHasFlags;
  const finishShapeOk = finishIndex < 0 || finishHasFlags;
  const canceled = attempted.some((cmd) => /\bbut resolve cancel\b/.test(cmd));

  return (
    enterIndex >= 0 &&
    statusIndex >= 0 &&
    enterShapeOk &&
    finishOrderingOk &&
    finishShapeOk &&
    !canceled &&
    !containsGitWrite(commands)
  );
}

export function pullCheckThenPullFlow(output: unknown): boolean {
  const data = parseOutput(output);
  const commands = commandStrings(data);
  const pullCheckIndex = commands.findIndex(
    (cmd) => /\bbut pull\b/.test(cmd) && /\s--check(\s|$)/.test(cmd),
  );
  const pullIndex = commands.findIndex(
    (cmd, idx) => idx > pullCheckIndex && /\bbut pull\b/.test(cmd) && !/\s--check(\s|$)/.test(cmd),
  );

  const pullCheckCmd = pullCheckIndex >= 0 ? commands[pullCheckIndex] : "";
  const pullCmd = pullIndex >= 0 ? commands[pullIndex] : "";
  const pullCheckHasJson = pullCheckCmd.includes("--json");
  const pullHasFlags = pullCmd.includes("--json") && pullCmd.includes("--status-after");

  return (
    pullCheckIndex >= 0 &&
    pullIndex > pullCheckIndex &&
    pullCheckHasJson &&
    pullHasFlags &&
    !containsGitWrite(commands)
  );
}

export function stackedPrCreationFlow(output: unknown): boolean {
  const data = parseOutput(output);
  const commands = commandStrings(data);
  const attempted = attemptedCommandStrings(data);
  const result = outputText(data);

  const prNewCommands = attempted.filter((cmd) => /\bbut pr new\b/.test(cmd));
  const profileBranch = branches(data).find((branch) => branch?.name === "profile-stack");
  const acceptedTargets = new Set<string>(["profile-stack"]);
  if (typeof profileBranch?.cliId === "string" && profileBranch.cliId.length > 0) {
    acceptedTargets.add(profileBranch.cliId);
  }
  const usesTopStackBranch = prNewCommands.some((cmd) => {
    const match = cmd.match(/\bbut pr new\s+([^\s-][^\s]*)/);
    if (!match) {
      return false;
    }
    return acceptedTargets.has(match[1]);
  });
  const usesGhPr = attempted.some((cmd) => /\bgh pr\b/.test(cmd));
  const authGuidancePresent =
    /\bbut config forge auth\b/i.test(result) ||
    /\bconfig forge auth\b/i.test(result) ||
    attempted.some((cmd) => /\bbut config forge auth\b/.test(cmd));

  return (
    prNewCommands.length > 0 &&
    usesTopStackBranch &&
    !usesGhPr &&
    !containsGitWrite(commands) &&
    (authGuidancePresent || !isErrorResult(data))
  );
}

export function stackedAnchorCommitFlow(output: unknown): boolean {
  const data = parseOutput(output);
  const commands = commandStrings(data);
  const unassigned = unassignedChanges(data);

  const anchorIndex = commands.findIndex(
    (cmd) =>
      /\bbut branch new profile-ui\b/.test(cmd) && /(?:\s-a|\s--anchor)\s+\S+/.test(cmd),
  );
  const commitIndex = commands.findIndex(
    (cmd) => /\bbut commit\b/.test(cmd) && !isHelpCommand(cmd),
  );
  const commitCmd = commitIndex >= 0 ? commands[commitIndex] : "";
  const commitIsTargeted =
    commitCmd.includes("--changes") ||
    /\s-p(\s|=)\S+/.test(commitCmd);

  const noiseStillUnassigned = unassigned.some((change) => change.filePath === "src/noise.rs");
  const profileNotUnassigned = !unassigned.some((change) => change.filePath === "src/profile.rs");
  const repoBranches = branches(data).map((branch) => branch.name);
  const hasAuthBaseBranch = repoBranches.includes("auth-base");
  const hasProfileBranch = repoBranches.includes("profile-ui");

  return (
    anchorIndex >= 0 &&
    commitIndex > anchorIndex &&
    commitIsTargeted &&
    !containsGitWrite(commands) &&
    hasRepoState(data) &&
    hasAuthBaseBranch &&
    hasProfileBranch &&
    noiseStillUnassigned &&
    profileNotUnassigned
  );
}
