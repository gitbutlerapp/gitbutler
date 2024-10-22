<script lang="ts">
	// import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { copyToClipboard } from '$lib/utils/clipboard';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranch, type Commit, type DetailedCommit } from '$lib/vbranches/types';
	import { getContext } from '@gitbutler/shared/context';

	interface Props {
		parent: ReturnType<typeof ContextMenu>;
		baseBranch: BaseBranch;
		branch: VirtualBranch | undefined;
		commit: DetailedCommit | Commit;
		commitUrl: string | undefined;
	}

	const { parent, baseBranch, branch, commit, commitUrl }: Props = $props();

	const branchController = getContext(BranchController);

	function insertBlankCommit(commitId: string, location: 'above' | 'below' = 'below') {
		if (!branch || !baseBranch) {
			console.error('Unable to insert commit');
			return;
		}
		branchController.insertBlankCommit(branch.id, commitId, location === 'above' ? -1 : 1);
	}
</script>

<ContextMenuSection>
	{#if commitUrl}
		<ContextMenuItem
			label="Open in browser"
			onclick={async () => {
				await openExternalUrl(commitUrl);
				parent.close();
			}}
		/>
		<ContextMenuItem
			label="Copy commit link"
			onclick={() => {
				copyToClipboard(commitUrl);
				parent.close();
			}}
		/>
	{/if}
	<ContextMenuItem
		label="Copy commit message"
		onclick={() => {
			copyToClipboard(commit.description);
			parent.close();
		}}
	/>
</ContextMenuSection>
{#if 'branchId' in commit}
	<ContextMenuSection>
		<ContextMenuItem
			label="Add empty commit above"
			onclick={() => {
				insertBlankCommit(commit.id, 'above');
				parent.close();
			}}
		/>
		<ContextMenuItem
			label="Add empty commit below"
			onclick={() => {
				insertBlankCommit(commit.id, 'below');
				parent.close();
			}}
		/>
	</ContextMenuSection>
{/if}
