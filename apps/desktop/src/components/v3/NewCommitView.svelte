<script lang="ts">
	import CommitMessageInput from '$components/v3/CommitMessageInput.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
	import { persistedCommitMessage } from '$lib/config/config';
	import { showError, showToast } from '$lib/notifications/toasts';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext, inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		stackId: string;
	};
	const { projectId, stackId }: Props = $props();

	const stackService = getContext(StackService);
	const [uiState, idSelection] = inject(UiState, IdSelection);
	const [createCommitInStack, commitCreation] = stackService.createCommit();

	const stackState = $derived(uiState.stack(stackId));
	const selected = $derived(stackState.selection.get());
	const branchName = $derived(selected.current?.branchName);
	const commitId = $derived(selected.current?.commitId);
	const changeSelection = getContext(ChangeSelectionService);
	const selection = $derived(changeSelection.list());
	const canCommit = $derived(branchName && selection.current.length > 0);
	const commitMessage = persistedCommitMessage(projectId, stackId);
	const [initialTitle, initialMessage] = $derived($commitMessage.split('\n\n'));

	let input = $state<ReturnType<typeof CommitMessageInput>>();
	let drawer = $state<ReturnType<typeof Drawer>>();

	async function createCommit(message: string) {
		if (!branchName) {
			throw new Error('No branch selected!');
		}
		const response = await createCommitInStack({
			projectId,
			stackId,
			parentId: commitId,
			message: message,
			stackBranchName: branchName,
			worktreeChanges: selection.current.map((item) =>
				item.type === 'full'
					? {
							pathBytes: item.pathBytes,
							hunkHeaders: []
						}
					: {
							pathBytes: item.pathBytes,
							hunkHeaders: item.hunks
						}
			)
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
			$commitMessage = '';
		}
	}

	function cancel() {
		drawer?.onClose();
	}
</script>

<Drawer bind:this={drawer} {projectId} {stackId} title="Create commit">
	<CommitMessageInput
		bind:this={input}
		{projectId}
		{stackId}
		actionLabel="Create commit"
		action={handleCommitCreation}
		onCancel={cancel}
		disabledAction={!canCommit}
		loading={commitCreation.current.isLoading}
		{initialTitle}
		{initialMessage}
		isNewCommit
	/>
</Drawer>
