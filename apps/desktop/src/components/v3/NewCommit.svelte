<script lang="ts">
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { CommitService } from '$lib/commits/commitService.svelte';
	import { showError } from '$lib/notifications/toasts';
	import { stackPath } from '$lib/routes/routes.svelte';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { code } from '@cartamd/plugin-code';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { Carta, MarkdownEditor } from 'carta-md';
	import DOMPurify from 'isomorphic-dompurify';
	import { goto } from '$app/navigation';
	import 'carta-md/default.css'; /* Default theme */
	import '@cartamd/plugin-code/default.css';

	const { projectId, stackId }: { projectId: string; stackId: string } = $props();

	const stackService = getContext(StackService);
	const commitService = getContext(CommitService);

	const baseBranchService = getContext(BaseBranchService);
	const base = $derived(baseBranchService.base);

	const changeSelection = getContext(ChangeSelectionService);
	const selection = $derived(changeSelection.list().current);

	/**
	 * The stackId parameter is currently optional, mainly so that we don't
	 * need a separate page for displaying an illustration. But it leads to
	 * this awkward derivation.
	 *
	 * TODO: Make stackId required.
	 */
	const branch = $derived(
		stackId ? stackService.getBranchByIndex(projectId, stackId, 0).current : undefined
	);

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

	/**
	 * Bound to the editor component.
	 */
	let commitMessage = $state('');

	function createCommit() {
		try {
			commitService.createCommit(projectId, {
				message: commitMessage,
				parentId: commitParent!,
				stackId,
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
		} catch (err: unknown) {
			showError('Failed to commit', err);
		}
	}

	/**
	 * The Carta theme is currently not reactive so we need to redraw the
	 * component if it changes. There is also no cleanup function in its
	 * API, so we probably need to upstream some changes.
	 *
	 * TODO: Make this better.
	 */
	let carta: Carta | undefined = $state();

	$effect(() => {
		carta = new Carta({
			sanitizer: DOMPurify.sanitize,
			extensions: [code()]
		});
	});
</script>

<div class="new-commit">
	<!-- See carta-md class in carta.scss for more styles for this div. -->
	<div class="carta-md">
		{#if carta}
			{#key carta}
				<MarkdownEditor
					bind:value={commitMessage}
					mode="tabs"
					theme="github"
					placeholder="Your commit summary"
					{carta}
				/>
			{/key}
		{/if}
	</div>
	<div class="actions">
		<Button
			kind="outline"
			style="neutral"
			width={96}
			onclick={() => goto(stackPath(projectId, stackId))}
		>
			Cancel
		</Button>
		<Button style="pop" wide onclick={createCommit}>Create commit!</Button>
	</div>
</div>

<style>
	.new-commit {
		display: flex;
		flex-direction: column;
		flex-grow: 1;
		height: 100%;
		background: var(--clr-bg-1);
	}
	.carta-md {
		flex-grow: 1;
		overflow: hidden;
	}
	.actions {
		display: flex;
		gap: 6px;
		padding: 16px;
		border-top: 1px solid var(--clr-border-2);
	}
</style>
