import { autoSelectBranchNameFeature, stagingBehaviorFeature } from "$lib/config/uiFeatureFlags";
import { StackService } from "$lib/stacks/stackService.svelte";
import { UiState } from "$lib/state/uiState.svelte";
import { WorktreeService } from "$lib/worktree/worktreeService.svelte";
import { InjectionToken } from "@gitbutler/core/context";
import { get } from "svelte/store";
import type { ProjectsService } from "$lib/project/projectsService";
import type { Stack } from "$lib/stacks/stack";
import type { EventProperties } from "$lib/state/customHooks.svelte";
import type { HunkAssignment } from "@gitbutler/but-sdk";
import type { Commit, Segment } from "@gitbutler/but-sdk";
import type { FModeManager } from "@gitbutler/ui/focus/fModeManager";

export const COMMIT_ANALYTICS = new InjectionToken<CommitAnalytics>("CommitAnalytics");

export class CommitAnalytics {
	constructor(
		private stackService: StackService,
		private uiState: UiState,
		private worktreeService: WorktreeService,
		private fModeManager: FModeManager,
		private projectsService: ProjectsService,
	) {}

	async getCommitProperties(args: {
		projectId: string;
		stackId: string;
		selectedBranchName: string;
		message: string;
		parentId?: string;
		isRichTextMode?: boolean;
	}): Promise<EventProperties> {
		try {
			// Fetch all data upfront
			const stacksResult = await this.stackService.fetchStacks(args.projectId);
			const stacks = stacksResult || [];

			const stackResult = await this.stackService.fetchStackById(args.projectId, args.stackId);
			const stack = stackResult;

			const branchesResult = await this.stackService.fetchBranches(args.projectId, args.stackId);
			const branches = branchesResult || [];

			const commitsResult = await this.stackService.fetchCommits(
				args.projectId,
				args.stackId,
				args.selectedBranchName,
			);
			const commits = commitsResult || [];

			const worktreeResult = await this.worktreeService.worktreeChanges.fetch({
				projectId: args.projectId,
			});
			const worktreeData = worktreeResult;

			if (!worktreeData) {
				throw new Error("Failed to fetch worktree data");
			}

			const assignments = worktreeData.hunkAssignments;

			const project = await this.projectsService.fetchProject(args.projectId);

			return {
				floatingCommitBox: this.uiState.global.useFloatingBox.current,
				// Whether the message editor was in rich-text mode (true) or plain-text mode (false)
				messageEditorRichTextMode: args.isRichTextMode || false,
				// Number of branches in the stack we are committing to
				branchCount: this.getBranchCount(stack),
				// Number of commits in the stack we are committing to
				laneCommitCount: this.getLaneCommitCount(branches),
				// Number of commits in the branch in the stack that we are committing to
				branchCommitCount: this.getBranchCommitCount(commits),
				// Whether this commit is the last/top commit in the branch
				isLastCommit: this.getIsLastCommit(branches, args.parentId),
				// Number of characters in the commit message
				messageCharacterCount: this.getMessageCharacterCount(args.message),
				// Number of new lines in the commit message
				messageNewLineCount: this.getMessageNewLineCount(args.message),
				// How many files were assigned to the lane where the commit is being created
				filesAssignedToCurrentLane: this.getFilesForStack(assignments, args.stackId).length,
				// How many lanes there are in the workspace
				totalLanesInWorkspace: stacks.length,
				// How many lanes in the workspace have assignments
				lanesWithAssignments: this.getLanesWithAssignments(stacks, assignments).length,
				// Total number of files that have been assigned to any lane in the workspace
				totalAssignedFiles: this.getAssignedFiles(assignments).length,
				// Total number of files that have not been assigned
				totalUnassignedFiles: this.getUnassignedFiles(assignments).length,
				// Number of times F key shortcuts have been "clicked"
				fKeyActivations: this.fModeManager.activations,
				// Whether gerrit mode is enabled for this project
				gerritMode: project.gerrit_mode,
				// Behavior metrics
				...this.getBehaviorMetrics(),
			};
		} catch (error) {
			console.error("Failed to fetch commit analytics:", error);
			return {};
		}
	}

	private getBranchCount(stack: Stack | undefined): number {
		return stack?.segments.length || 0;
	}

	private getLaneCommitCount(branches: Segment[]): number {
		return branches.reduce((total, branch) => total + branch.commits.length, 0);
	}

	private getBranchCommitCount(commits: Commit[]): number {
		return commits.length;
	}

	private getIsLastCommit(branches: Segment[], parentId: string | undefined): boolean {
		// If there is no parent, then this is implicitly the top of the stack.
		if (!parentId) {
			return true;
		}

		const topBranch = branches.at(0);
		return (topBranch?.commits.at(0)?.id ?? topBranch?.base) === parentId;
	}

	private getMessageCharacterCount(message: string): number {
		return message.length;
	}

	private getMessageNewLineCount(message: string): number {
		return message.match(/\n/g)?.length || 0;
	}

	private getFilesForStack(assignments: HunkAssignment[], stackId: string): string[] {
		const paths = new Set<string>();
		assignments
			.filter((assignment) => assignment.stackId === stackId)
			.forEach((assignment) => paths.add(assignment.path));
		return Array.from(paths);
	}

	private getLanesWithAssignments(stacks: Stack[], assignments: HunkAssignment[]): Stack[] {
		const assignedStacks = new Set<string>();
		assignments.forEach((assignment) => {
			if (assignment.stackId !== null) {
				assignedStacks.add(assignment.stackId);
			}
		});

		return stacks.filter((stack) => stack.id && assignedStacks.has(stack.id));
	}

	private getAssignedFiles(assignments: HunkAssignment[]): string[] {
		const paths = new Set<string>();
		assignments
			.filter((assignment) => assignment.stackId !== null)
			.forEach((assignment) => paths.add(assignment.path));
		return Array.from(paths);
	}

	private getUnassignedFiles(assignments: HunkAssignment[]): string[] {
		const paths = new Set<string>();
		assignments
			.filter((assignment) => assignment.stackId === null)
			.forEach((assignment) => paths.add(assignment.path));
		return Array.from(paths);
	}

	private getBehaviorMetrics(): EventProperties {
		// Placeholder for future behavior metrics
		const stagingBehavior = get(stagingBehaviorFeature);
		const autoSelectBranchName = get(autoSelectBranchNameFeature);
		const behaviorMetrics = {
			stagingBehavior,
			autoSelectBranchName,
		};

		return namespaceProps(behaviorMetrics, "behavior");
	}
}

function namespaceProps(props: EventProperties, namespace: string): EventProperties {
	const namespacedProps: EventProperties = {};
	for (const [key, value] of Object.entries(props)) {
		namespacedProps[`${namespace}:${key}`] = value;
	}
	return namespacedProps;
}
