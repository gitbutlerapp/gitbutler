<script lang="ts">
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { isLockfile } from '@gitbutler/shared/lockfiles';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import type CommitSuggestions from '$components/v3/editor/commitSuggestions.svelte';
	import type { DiffInput } from '$lib/ai/service';
	import type { DiffHunk } from '$lib/hunks/hunk';

	type Props = {
		canUseAI: boolean;
		projectId: string;
		suggestionsHandler: CommitSuggestions;
		existingCommitId: string | undefined;
	};

	const { suggestionsHandler, projectId, canUseAI, existingCommitId }: Props = $props();

	const worktreeService = getContext(WorktreeService);
	const diffService = getContext(DiffService);
	const changeSelection = getContext(ChangeSelectionService);
	const stackService = getContext(StackService);

	const selectedFiles = $derived(changeSelection.list().current);
	const selectedPaths = $derived(selectedFiles.map((file) => file.path));

	const changes = $derived.by(() => {
		if (existingCommitId) return stackService.commitChanges(projectId, existingCommitId);
		return worktreeService.getChangesById(projectId, selectedPaths);
	});
	const treeChanges = $derived(changes?.current.data);
	const changeDiffsResponse = $derived(
		treeChanges ? diffService.getChanges(projectId, treeChanges) : undefined
	);
	const changeDiffs = $derived(
		changeDiffsResponse?.current.map((item) => item.data).filter(isDefined) ?? []
	);

	$effect(() => {
		suggestionsHandler.setStagedChanges(changeDiffs);
	});

	$effect(() => {
		suggestionsHandler.setCanUseAI(canUseAI);
	});

	/**
	 * Determine whether the file path should be skipped when passing the diff to the AI service.
	 */
	function shouldSkipFilePath(filePath: string): boolean {
		if (isLockfile(filePath)) {
			// Lockfiles should be skipped.
			return true;
		}
		if (existingCommitId) {
			// We're generating the commit message for an existing commit.
			// No changes should be skipped.
			return false;
		}
		const selectedFile = selectedFiles.find((file) => file.path === filePath);
		return selectedFile === undefined;
	}

	/**
	 * Determine whether the hunk in the current path should be skipped when passing the diff to the AI service.
	 */
	function shouldSkipHunk(filePath: string, hunk: DiffHunk): boolean {
		if (isLockfile(filePath)) {
			// Lockfiles should be skipped.
			return true;
		}
		if (existingCommitId) {
			// We're generating the commit message for an existing commit.
			// No hunks should be skipped.
			return false;
		}

		const selectedFile = selectedFiles.find((file) => file.path === filePath);
		if (selectedFile === undefined) {
			return true;
		}
		if (selectedFile.type === 'full') {
			return false;
		}

		if (selectedFile.hunks.length === 0) {
			// We assume that if the hunks are empty, the whole file is selected
			return false;
		}

		const selectedHunk = selectedFile.hunks.find(
			(selectedHunk) =>
				selectedHunk.oldStart === hunk.oldStart &&
				selectedHunk.oldLines === hunk.oldLines &&
				selectedHunk.newStart === hunk.newStart &&
				selectedHunk.newLines === hunk.newLines
		);

		return selectedHunk === undefined;
	}

	/**
	 * Get the selected diff information in the expected format for the AI service.
	 *
	 * TODO: Account for the line selection. Right now, it will always send the whole hunk.
	 */
	export function getDiffInput(): DiffInput[] {
		const diffInput: DiffInput[] = [];

		for (const diff of changeDiffs) {
			const filePath = diff.path;
			const diffStringBuffer: string[] = [];

			if (diff.diff.type !== 'Patch') continue;
			if (shouldSkipFilePath(filePath)) continue;

			for (const hunk of diff.diff.subject.hunks) {
				if (shouldSkipHunk(filePath, hunk)) continue;
				diffStringBuffer.push(hunk.diff);
			}

			const diffString = diffStringBuffer.join('\n');
			diffInput.push({
				filePath,
				diff: diffString
			});
		}
		return diffInput;
	}
</script>
