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

	type Props = { projectId: string; stackId: string; branchName: string };
	const { projectId, stackId, branchName }: Props = $props();

	const baseBranchService = getContext(BaseBranchService);
	const stackService = getContext(StackService);
	const base = $derived(baseBranchService.base);

	const changeSelection = getContext(ChangeSelectionService);
	const selection = $derived(changeSelection.list().current);

	/**
	 * Toggles use of markdown on/off in the message editor.
	 */
	let markdown = persisted(true, 'useMarkdown__' + projectId);

	const commitsResult = $derived(stackService.commitAt(projectId, stackId, branchName, 0).current);
	const topCommit = $derived(commitsResult.data);

	/**
	 * At the moment this code can only commit to the tip of the stack.
	 *
	 * TODO: Implement according to design.
	 */
	const commitParent = $derived(topCommit ? topCommit.id : $base?.baseSha);
	let composer: CommitMessageEditor | undefined = $state();

	/**
	 * TODO: Is there a way of getting the value synchronously?
	 */
	function createCommit() {
		composer?.getPlaintext(async (message) => {
			try {
				await _createCommit(message);
			} catch (err: unknown) {
				showError('Failed to commit', err);
			}
		});
	}

	async function _createCommit(message: string) {
		await stackService.createCommit(projectId, {
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
