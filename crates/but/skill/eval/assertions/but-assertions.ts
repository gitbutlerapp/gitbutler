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
  name?: unknown;
  commits?: RepoCommit[];
};

type RepoStack = {
  branches?: RepoBranch[];
};

type EvalOutput = {
  commands?: CommandTrace[];
  repoState?: {
    unassignedChanges?: RepoChange[];
    stacks?: RepoStack[];
  } | null;
  repoStateError?: unknown;
};

const GIT_WRITE_RE = /\bgit (add|commit|push|merge|rebase|checkout)\b/;
const GIT_WRITE_NO_REBASE_RE = /\bgit (add|commit|push|merge|checkout)\b/;

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
    .map((entry) => (typeof entry?.command === "string" ? entry.command : ""));
}

function hasTargetedButCommit(commands: string[]): boolean {
  const commitCmds = commands.filter((cmd) => cmd.includes("but commit"));
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
  const mutationCmds = commands.filter((cmd) => matcher.test(cmd));
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

export function basicCommitFlow(output: unknown): boolean {
  const data = parseOutput(output);
  const commands = commandStrings(data);

  const statusIndex = commands.findIndex((cmd) => cmd.includes("but status"));
  const commitIndex = commands.findIndex((cmd) => cmd.includes("but commit"));
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
  const commitIndex = commands.findIndex((cmd) => cmd.includes("but commit"));
  const commitCmds = commands.filter((cmd) => cmd.includes("but commit"));
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
  const commitIndex = commands.findIndex((cmd) => cmd.includes("but commit"));
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
  const amendIndex = commands.findIndex((cmd) => cmd.includes("but amend"));
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
