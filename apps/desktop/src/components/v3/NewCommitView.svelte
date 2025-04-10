<script lang="ts">
	import CommitMessageInput from '$components/v3/CommitMessageInput.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import { lineIdsToHunkHeaders, type DiffHunk, type HunkHeader } from '$lib/hunks/hunk';
	import { showError, showToast } from '$lib/notifications/toasts';
	import { ChangeSelectionService, type SelectedHunk } from '$lib/selection/changeSelection.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import {
		StackService,
		type CreateCommitRequestWorktreeChanges
	} from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { getContext, inject } from '@gitbutler/shared/context';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';

	type Props = {
		projectId: string;
		stackId?: string;
	};
	const { projectId, stackId }: Props = $props();

	const stackService = getContext(StackService);
	const [uiState, idSelection, worktreeService, diffService] = inject(
		UiState,
		IdSelection,
		WorktreeService,
		DiffService
	);
	const changeSelection = getContext(ChangeSelectionService);

	const [createCommitInStack, commitCreation] = stackService.createCommit;

	const stackState = $derived(stackId ? uiState.stack(stackId) : undefined);
	const selected = $derived(stackState?.selection.get());
	const branchName = $derived(selected?.current?.branchName);
	const commitId = $derived(selected?.current?.commitId);
	const selection = $derived(changeSelection.list());

	const selectedPaths = $derived(selection.current.map((item) => item.path));
	const selectedTreeChangesResponse = $derived(
		worktreeService.getChangesById(projectId, selectedPaths)
	);
	const selectedTreeChanges = $derived(selectedTreeChangesResponse.current.data);
	const selectedChangesResponse = $derived(
		selectedTreeChanges ? diffService.getChanges(projectId, selectedTreeChanges) : undefined
	);
	const changeDiffs = $derived(
		selectedChangesResponse?.current.map((item) => item.data).filter(isDefined) ?? []
	);

	const canCommit = $derived(branchName && selection.current.length > 0);
	const projectState = $derived(uiState.project(projectId));

	let input = $state<ReturnType<typeof CommitMessageInput>>();
	let drawer = $state<ReturnType<typeof Drawer>>();

	function findHunkDiff(filePath: string, hunk: SelectedHunk): DiffHunk | undefined {
		const file = changeDiffs.find((file) => file.path === filePath);
		if (!file) return undefined;
		if (file.diff.type !== 'Patch') return undefined;

		const hunkDiff = file.diff.subject.hunks.find(
			(hunkDiff) =>
				hunkDiff.oldStart === hunk.oldStart &&
				hunkDiff.oldLines === hunk.oldLines &&
				hunkDiff.newStart === hunk.newStart &&
				hunkDiff.newLines === hunk.newLines
		);
		return hunkDiff;
	}

	async function createCommit(message: string) {
		if (!stackId) {
			throw new Error('No stack selected!');
		}

		if (!branchName) {
			throw new Error('No branch selected!');
		}

		if (!selectedTreeChanges) {
			throw new Error('No changes selected!');
		}

		const worktreeChanges: CreateCommitRequestWorktreeChanges[] = [];

		for (const item of selection.current) {
			if (item.type === 'full') {
				worktreeChanges.push({
					pathBytes: item.pathBytes,
					hunkHeaders: []
				});
				continue;
			}

			if (item.type === 'partial') {
				const hunkHeaders: HunkHeader[] = [];
				for (const hunk of item.hunks) {
					if (hunk.type === 'full') {
						hunkHeaders.push(hunk);
						continue;
					}

					if (hunk.type === 'partial') {
						const hunkDiff = findHunkDiff(item.path, hunk);
						if (!hunkDiff) {
							throw new Error('Hunk not found while commiting');
						}
						const selectedLines = hunk.lines;
						hunkHeaders.push(...lineIdsToHunkHeaders(selectedLines, hunkDiff.diff, 'commit'));
						continue;
					}
				}
				worktreeChanges.push({
					pathBytes: item.pathBytes,
					hunkHeaders
				});
				continue;
			}
		}

		const response = await createCommitInStack({
			projectId,
			stackId,
			parentId: commitId,
			message: message,
			stackBranchName: branchName,
			worktreeChanges
		});

		if (!response.data) {
			showToast({ message: 'Failed to create commit', style: 'error' });
			return;
		}

		const newId = response.data.newCommit;

		uiState.project(projectId).drawerPage.set(undefined);
		uiState.stack(stackId).selection.set({ branchName, commitId: newId });
		changeSelection.clear();
		idSelection.clear({ type: 'worktree' });
	}

	async function handleCommitCreation() {
		const message = input?.getMessage();
		if (!message) {
			showToast({ message: 'Commit message is required', style: 'error' });
			return;
		}

		try {
			await createCommit(message);
		} catch (err: unknown) {
			showError('Failed to commit', err);
		} finally {
			projectState.commitTitle.set('');
			projectState.commitDescription.set('');
		}
	}

	function cancel() {
		drawer?.onClose();
	}
</script>

<Drawer bind:this={drawer} {projectId} {stackId} title="Create commit" disableScroll minHeight={20}>
	<CommitMessageInput
		bind:this={input}
		{projectId}
		{stackId}
		actionLabel="Create commit"
		action={handleCommitCreation}
		onCancel={cancel}
		disabledAction={!canCommit}
		loading={commitCreation.current.isLoading}
	/>
</Drawer>
