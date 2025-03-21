<script lang="ts">
	import CommitMessageInput from '$components/v3/CommitMessageInput.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
	import { showError } from '$lib/notifications/toasts';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext, inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		stackId: string;
	};
	const { projectId, stackId }: Props = $props();

	const stackService = getContext(StackService);
	const [uiState] = inject(UiState);
	const [createCommitInStack, commitCreation] = stackService.createCommit();

	const stackState = $derived(uiState.stack(stackId));
	const selected = $derived(stackState.selection.get());
	const branchName = $derived(selected.current?.branchName);
	const commitId = $derived(selected.current?.commitId);
	const changeSelection = getContext(ChangeSelectionService);
	const selection = $derived(changeSelection.list());
	const canCommit = $derived(branchName && selection.current.length > 0);

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

		const newId = response.newCommit;

		uiState.project(projectId).drawerPage.set(undefined);
		uiState.stack(stackId).selection.set({ branchName, commitId: newId });
	}

	async function hanldleCommitCreation() {
		const titleText = await input?.getTitle();
		const message = await input?.getPlaintext();
		if (!message && !titleText) return;

		const commitMessage = [titleText, message].filter((a) => a).join('\n\n');

		try {
			await createCommit(commitMessage);
		} catch (err: unknown) {
			showError('Failed to commit', err);
		}
	}

	function cancel() {
		drawer?.onClose();
	}
</script>

<Drawer bind:this={drawer} {projectId} {stackId}>
	{#snippet header()}
		<p class="text-14 text-semibold">Create commit</p>
	{/snippet}
	<CommitMessageInput
		{projectId}
		action={hanldleCommitCreation}
		actionLabel="Commit"
		onCancel={cancel}
		disabledAction={!canCommit}
		loading={commitCreation.current.isLoading}
	/>
</Drawer>
