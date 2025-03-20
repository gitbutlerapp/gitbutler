<script lang="ts">
	import Drawer from '$components/v3/Drawer.svelte';
	import EditorFooter from '$components/v3/editor/EditorFooter.svelte';
	import MessageEditor from '$components/v3/editor/MessageEditor.svelte';
	import { showError } from '$lib/notifications/toasts';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext, inject } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Button from '@gitbutler/ui/Button.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';

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

	let titleText = $state<string>();

	/**
	 * Toggles use of markdown on/off in the message editor.
	 */
	let markdown = persisted(true, 'useMarkdown__' + projectId);

	let composer = $state<ReturnType<typeof MessageEditor>>();
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
		const message = await composer?.getPlaintext();
		if (!message && !titleText) return;

		const commitMessage = [message, titleText].filter((a) => a).join('\n\n');

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
	<div class="new-commit-fields">
		<Textbox bind:value={titleText} placeholder="Commit title" />
		<MessageEditor bind:this={composer} bind:markdown={$markdown} />
	</div>
	<EditorFooter onCancel={cancel}>
		<Button
			style="pop"
			onclick={hanldleCommitCreation}
			disabled={!canCommit}
			loading={commitCreation.current.isLoading}>Create commit</Button
		>
	</EditorFooter>
</Drawer>

<style lang="postcss">
	.new-commit-fields {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
</style>
