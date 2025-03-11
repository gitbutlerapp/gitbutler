<script lang="ts">
	import CommitMessageEditor from './editor/CommitMessageEditor.svelte';
	import EditorFooter from './editor/EditorFooter.svelte';
	import EditorHeader from './editor/EditorHeader.svelte';
	import { showError } from '$lib/notifications/toasts';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext, inject } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Button from '@gitbutler/ui/Button.svelte';

	type Props = {
		projectId: string;
		stackId: string;
	};
	const { projectId, stackId }: Props = $props();

	const stackService = getContext(StackService);
	const [uiState] = inject(UiState);

	const selected = $derived(uiState.stack(stackId).selection.get());
	const branchName = $derived(selected.current?.branchName);
	const commitId = $derived(selected.current?.commitId);
	const canCommit = $derived(branchName && commitId);
	const changeSelection = getContext(ChangeSelectionService);
	const selection = $derived(changeSelection.list());

	/**
	 * Toggles use of markdown on/off in the message editor.
	 */
	let markdown = persisted(true, 'useMarkdown__' + projectId);

	let composer: CommitMessageEditor | undefined = $state();

	/**
	 * TODO: Is there a way of getting the value synchronously?
	 */
	async function createCommit() {
		const message = await composer?.getPlaintext();
		if (!message) return;

		try {
			await _createCommit(message);
		} catch (err: unknown) {
			showError('Failed to commit', err);
		}
	}

	async function _createCommit(message: string) {
		if (!branchName) {
			throw new Error('No branch selected!');
		}
		if (!commitId) {
			throw new Error('No commit selected!');
		}
		const response = await stackService.createCommit(projectId, {
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
		if (response.error) {
			throw response.error;
		}
		const newId = response.data?.newCommit;
		if (newId) {
			uiState.project(projectId).drawerPage.set(undefined);
			uiState.stack(stackId).selection.set({ branchName, commitId: newId });
		}
	}
</script>

<EditorHeader title="New commit" bind:markdown={$markdown} />
<CommitMessageEditor bind:this={composer} bind:markdown={$markdown} />
<EditorFooter onCancel={() => uiState.project(projectId).drawerPage.set(undefined)}>
	<Button style="pop" onclick={createCommit} wide disabled={!canCommit}>Create commit</Button>
</EditorFooter>
