<script lang="ts">
	import CommitMessageEditor from './editor/CommitMessageEditor.svelte';
	import EditorFooter from './editor/EditorFooter.svelte';
	import EditorHeader from './editor/EditorHeader.svelte';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { showError } from '$lib/notifications/toasts';
	import { stackPath } from '$lib/routes/routes.svelte';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Button from '@gitbutler/ui/Button.svelte';
	import { goto } from '$app/navigation';

	const { projectId, stackId }: { projectId: string; stackId: string } = $props();

	const baseBranchService = getContext(BaseBranchService);
	const stackService = getContext(StackService);
	const base = $derived(baseBranchService.base);

	const changeSelection = getContext(ChangeSelectionService);
	const selection = $derived(changeSelection.list().current);

	/**
	 * Toggles use of markdown on/off in the message editor.
	 */
	let markdown = persisted(true, 'useMarkdown__' + projectId);

	/**
	 * Commit message placeholder text.
	 *
	 * TODO: Make stackId required.
	 */
	const branch = $derived(stackService.getBranchByIndex(projectId, stackId, 0).current);

	/**
	 * TODO: Find a better way of accessing top commit.
	 */
	const commit = $derived(
		branch && branch.data?.state.type === 'Stacked'
			? branch.data.state.subject.localAndRemote.at(0)
			: undefined
	);

	/**
	 * At the moment this code can only commit to the tip of the stack.
	 *
	 * TODO: Implement according to design.
	 */
	const commitParent = $derived(commit ? commit.id : $base?.baseSha);
	let composer: CommitMessageEditor | undefined = $state();

	/**
	 * TODO: Is there a way of getting the value synchronously?
	 */
	function createCommit() {
		console.log(composer);
		composer?.getPlaintext((message) => {
			try {
				_createCommit(message);
			} catch (err: unknown) {
				showError('Failed to commit', err);
			}
		});
	}

	function _createCommit(message: string) {
		console.log(message, commitParent, selection);
		stackService.createCommit(projectId, {
			stackId,
			parentId: commitParent!,
			message: message,
			worktreeChanges: selection.map((item) =>
				item.type === 'full'
					? {
							pathBytes: item.pathBytes,
							previousPathBytes: item.previousPathBytes,
							hunkHeaders: []
						}
					: {
							pathBytes: item.pathBytes,
							hunkHeaders: item.hunks
						}
			)
		});
		goto(stackPath(projectId, stackId));
	}
</script>

<div class="new-commit">
	<EditorHeader title="New commit" bind:markdown={$markdown} />
	<CommitMessageEditor bind:this={composer} bind:markdown={$markdown} />
	<EditorFooter onCancel={() => goto(stackPath(projectId, stackId))}>
		<Button style="pop" onclick={createCommit} wide>Create commit</Button>
	</EditorFooter>
</div>

<style>
	.new-commit {
		display: flex;
		flex-direction: column;
		flex-grow: 1;
		height: 100%;
		background: var(--clr-bg-1);
	}
</style>
