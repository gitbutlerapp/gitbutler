import { compactWorkspace } from '$lib/config/uiFeatureFlags';
import { StackService } from '$lib/stacks/stackService.svelte';
import { UiState } from '$lib/state/uiState.svelte';
import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
import { get } from 'svelte/store';
import type { Commit } from '$lib/branches/v3';
import type { HunkAssignment } from '$lib/hunks/hunk';
import type { Stack, BranchDetails } from '$lib/stacks/stack';
import type { EventProperties } from '$lib/state/customHooks.svelte';

export class CommitAnalytics {
	constructor(
		private stackService: StackService,
		private uiState: UiState,
		private worktreeService: WorktreeService
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
			const stacks = stacksResult.data || [];

			const stackResult = await this.stackService.fetchStackById(args.projectId, args.stackId);
			const stack = stackResult.data;

			const branchesResult = await this.stackService.fetchBranches(args.projectId, args.stackId);
			const branches = branchesResult.data || [];

			const commitsResult = await this.stackService.fetchCommits(
				args.projectId,
				args.stackId,
				args.selectedBranchName
			);
			const commits = commitsResult.data || [];

			const worktreeResult = await this.worktreeService.worktreeChanges.fetch({
				projectId: args.projectId
			});
			const worktreeData = worktreeResult.data;

			if (!worktreeData) {
				throw new Error('Failed to fetch worktree data');
			}

			const assignments = worktreeData.hunkAssignments;

			return {
				floatingCommitBox: this.uiState.global.useFloatingBox.current,
				// Whether the message editor was in rich-text mode (true) or plain-text mode (false)
				messageEditorRichTextMode: args.isRichTextMode || false,
				// Number of branches in the stack we are committing to
				branchCount: this.getBranchCount(stack),
				// Number of commits in the stack we are committing to
				laneCommitCount: this.getLaneCommitCount(branches),
				// Number of commits in the branch in the stack that we are comitting to
				branchCommitCount: this.getBranchCommitCount(commits),
				// Whether this commit is the last/top commit in the branch
				isLastCommit: this.getIsLastCommit(stack, args.parentId),
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
				// Compact preview mode enabled
				compactPreview: get(compactWorkspace)
			};
		} catch (error) {
			console.error('Failed to fetch commit analytics:', error);
			return {};
		}
	}

	private getBranchCount(stack: Stack | undefined): number {
		return stack?.heads.length || 0;
	}

	private getLaneCommitCount(branches: BranchDetails[]): number {
		return branches.reduce((total, branch) => total + branch.commits.length, 0);
	}

	private getBranchCommitCount(commits: Commit[]): number {
		return commits.length;
	}

	private getIsLastCommit(stack: Stack | undefined, parentId: string | undefined): boolean {
		// If there is no parent, then this is implicitly the top of the stack.
		if (!parentId) {
			return true;
		}

		return stack?.tip === parentId;
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
		assignments
			.filter((assignment) => assignment.stackId !== null)
			.forEach((assignment) => assignedStacks.add(assignment.stackId!));

		return stacks.filter((stack) => assignedStacks.has(stack.id));
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
}
