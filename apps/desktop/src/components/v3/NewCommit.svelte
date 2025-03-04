<script lang="ts">
	import CommitGoesHere from './CommitGoesHere.svelte';
	import CommitMessageEditor from './editor/CommitMessageEditor.svelte';
	import EditorFooter from './editor/EditorFooter.svelte';
	import EditorHeader from './editor/EditorHeader.svelte';
	import ResizeableSplitLayout from '$components/v3/ResizeableSplitLayout.svelte';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { showError } from '$lib/notifications/toasts';
	import { commitPath, stackPath } from '$lib/routes/routes.svelte';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Button from '@gitbutler/ui/Button.svelte';
	import { goto } from '$app/navigation';

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
		// The parent of the new commit.
		commitId?: string;
	};
	const { projectId, stackId, branchName, commitId }: Props = $props();

	const baseBranchService = getContext(BaseBranchService);
	const stackService = getContext(StackService);
	const base = $derived(baseBranchService.base);

	const changeSelection = getContext(ChangeSelectionService);
	const selection = $derived(changeSelection.list().current);

	/**
	 * Toggles use of markdown on/off in the message editor.
	 */
	let markdown = persisted(true, 'useMarkdown__' + projectId);

	const commitResult = $derived(stackService.commitAt(projectId, stackId, branchName, 0).current);
	const commit = $derived(commitResult.data);

	const baseSha = $derived($base?.baseSha);
	const defaultParentId = $derived(commit ? commit.id : baseSha);
	const parentId = $derived(commitId ? commitId : defaultParentId!);

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
		const response = await stackService.createCommit(projectId, {
			stackId,
			parentId,
			message: message,
			stackBranchName: branchName,
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
		if (response.error) {
			throw response.error;
		}
		const newId = response.data?.newCommit;
		if (newId) {
			goto(commitPath(projectId, { stackId, branchName, commitId: newId, upstream: false }));
		}
	}
</script>

<ResizeableSplitLayout {projectId}>
	{#snippet main()}
		<EditorHeader title="New commit" bind:markdown={$markdown} />
		<CommitMessageEditor bind:this={composer} bind:markdown={$markdown} />
		<EditorFooter onCancel={() => goto(stackPath(projectId, stackId))}>
			<Button style="pop" onclick={createCommit} wide>Create commit</Button>
		</EditorFooter>
	{/snippet}
	{#snippet right()}
		<CommitGoesHere {projectId} {stackId} {branchName} {parentId} />
	{/snippet}
</ResizeableSplitLayout>
