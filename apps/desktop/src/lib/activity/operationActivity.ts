import { writable } from "svelte/store";

export type GitOperationProgress = {
	operation: string;
	phase: string;
	phaseLabel: string;
	elapsedMs: number;
	detail?: string;
	path?: string;
};

type ActivitySource = "backend" | "git-progress";

export type OperationActivity = {
	id: string;
	source: ActivitySource;
	command?: string;
	projectId?: string;
	operation?: string;
	label: string;
	detail?: string;
	startedAt: number;
	updatedAt: number;
};

type OperationActivityState = {
	activities: OperationActivity[];
};

let nextId = 0;
const gitProgressTimers = new Map<string, ReturnType<typeof setTimeout>>();

export const operationActivityStore = writable<OperationActivityState>({ activities: [] });

export function beginBackendOperation(command: string, params?: Record<string, unknown>) {
	const id = `backend:${++nextId}`;
	const startedAt = Date.now();
	const label = commandLabel(command, params);
	operationActivityStore.update((state) => ({
		activities: [
			...state.activities,
			{
				id,
				source: "backend",
				command,
				label,
				detail: commandDetail(command),
				startedAt,
				updatedAt: startedAt,
			},
		],
	}));

	return {
		finish: () => removeActivity(id),
	};
}

export function recordGitOperationProgress(projectId: string, progress: GitOperationProgress) {
	const id = `git-progress:${projectId}:${progress.operation}`;
	const now = Date.now();
	clearProgressTimer(id);

	operationActivityStore.update((state) => {
		const existing = state.activities.find((activity) => activity.id === id);
		const activity = {
			id,
			source: "git-progress" as const,
			projectId,
			operation: progress.operation,
			label: progress.phaseLabel,
			detail: progress.detail ?? progress.path,
			startedAt: existing?.startedAt ?? now - progress.elapsedMs,
			updatedAt: now,
		};
		return {
			activities: [...state.activities.filter((activity) => activity.id !== id), activity],
		};
	});

	if (progress.phase === "complete" || progress.phase === "failed") {
		gitProgressTimers.set(id, setTimeout(() => removeActivity(id), 1200));
	}
}

function removeActivity(id: string) {
	clearProgressTimer(id);
	gitProgressTimers.delete(id);
	operationActivityStore.update((state) => ({
		activities: state.activities.filter((activity) => activity.id !== id),
	}));
}

function clearProgressTimer(id: string) {
	const timer = gitProgressTimers.get(id);
	if (timer) clearTimeout(timer);
}

function commandLabel(command: string, params?: Record<string, unknown>): string {
	if (command === "acp_prompt") {
		const agentName = agentNameFromParams(params);
		return agentName ? `Asking ${agentName}` : "Asking AI";
	}

	const labels: Record<string, string> = {
		absorb: "Absorbing changes",
		add_project: "Adding repository",
		add_project_best_effort: "Adding repository",
		autofix_unity_project: "Checking Unity project",
		changes_in_worktree: "Loading changed files",
		commit_amend: "Amending commit",
		commit_create: "Creating commit",
		commit_insert_blank: "Creating commit",
		commit_move: "Moving commit",
		commit_move_changes_between: "Moving changes",
		commit_reword: "Updating commit message",
		commit_squash: "Squashing commits",
		commit_uncommit: "Uncommitting",
		commit_uncommit_changes: "Uncommitting changes",
		discard_worktree_changes: "Discarding changes",
		enter_edit_mode: "Opening edit mode",
		fetch_from_remotes: "Fetching remotes",
		integrate_upstream: "Integrating upstream",
		list_projects: "Loading repositories",
		restore_snapshot: "Restoring snapshot",
		save_edit_and_return_to_workspace: "Returning to workspace",
		set_local_ignored_path: "Updating local ignore",
		set_project_active: "Opening repository",
		stash_into_branch: "Stashing changes",
		stacks: "Loading branches",
		switch_back_to_workspace: "Switching workspace",
		tree_change_diffs: "Loading diff",
	};

	return labels[command] ?? sentenceCase(command);
}

function commandDetail(command: string): string | undefined {
	if (command === "acp_prompt") return "Waiting for the external agent to respond.";
	if (command === "changes_in_worktree") return "Scanning the worktree and assignments.";
	if (command === "commit_amend") return "Rewriting the commit and refreshing changed files.";
	if (command === "commit_uncommit") return "Moving committed changes back to the workspace.";
	if (command === "commit_create") return "Writing a local commit.";
	if (command === "set_project_active") return "Opening the repository and starting watchers.";
	return undefined;
}

function agentNameFromParams(params?: Record<string, unknown>): string | undefined {
	const agent = params?.agent;
	if (!agent || typeof agent !== "object" || !("name" in agent)) return undefined;
	const name = agent.name;
	return typeof name === "string" ? name : undefined;
}

function sentenceCase(command: string): string {
	const spaced = command.replaceAll("_", " ");
	return spaced.charAt(0).toUpperCase() + spaced.slice(1);
}
