<script lang="ts">
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
		menu: ReturnType<typeof ContextMenu> | undefined;
		leftClickTrigger: HTMLElement | undefined;
		rightClickTrigger: HTMLElement | undefined;
		baseBranch: BaseBranch;
		branch: VirtualBranch | undefined;
		commit: DetailedCommit | Commit;
		commitUrl: string | undefined;
		isRemote: boolean;
		onUncommitClick: (event: MouseEvent) => void;
		onEditMessageClick: (event: MouseEvent) => void;
		onPatchEditClick: (event: MouseEvent) => void;
		onClose?: () => void;
		onToggle?: (isOpen: boolean, isLeftClick: boolean) => void;
	}

	let {
		menu = $bindable(),
		leftClickTrigger,
		rightClickTrigger,
		baseBranch,
		branch,
		commit,
		commitUrl,
		isRemote,
		onUncommitClick,
		onEditMessageClick,
		onPatchEditClick,
		onClose,
		onToggle
	}: Props = $props();

	const branchController = getContext(BranchController);

	function insertBlankCommit(commitId: string, location: 'above' | 'below' = 'below') {
		if (!branch || !baseBranch) {
			console.error('Unable to insert commit');
			return;
		}
		branchController.insertBlankCommit(branch.id, commitId, location === 'above' ? -1 : 1);
	}
</script>

<ContextMenu
	bind:this={menu}
	{leftClickTrigger}
	{rightClickTrigger}
	onclose={onClose}
	ontoggle={onToggle}
>
	{#if !isRemote}
		<ContextMenuSection>
			<ContextMenuItem
				label="Uncommit"
				onclick={(e: MouseEvent) => {
					onUncommitClick(e);
					menu?.close();
				}}
			/>
			<ContextMenuItem
				label="Edit commit message"
				onclick={(e: MouseEvent) => {
					onEditMessageClick(e);
					menu?.close();
				}}
			/>
			<ContextMenuItem
				label="Edit commit"
				onclick={(e: MouseEvent) => {
					onPatchEditClick(e);
					menu?.close();
				}}
			/>
		</ContextMenuSection>
	{/if}
	<ContextMenuSection>
		{#if commitUrl}
			<ContextMenuItem
				label="Open in browser"
				onclick={async () => {
					await openExternalUrl(commitUrl);
					menu?.close();
				}}
			/>
			<ContextMenuItem
				label="Copy commit link"
				onclick={() => {
					copyToClipboard(commitUrl);
					menu?.close();
				}}
			/>
		{/if}
		<ContextMenuItem
			label="Copy commit message"
			onclick={() => {
				copyToClipboard(commit.description);
				menu?.close();
			}}
		/>
	</ContextMenuSection>
	{#if 'branchId' in commit && !isRemote}
		<ContextMenuSection>
			<ContextMenuItem
				label="Add empty commit above"
				onclick={() => {
					insertBlankCommit(commit.id, 'above');
					menu?.close();
				}}
			/>
			<ContextMenuItem
				label="Add empty commit below"
				onclick={() => {
					insertBlankCommit(commit.id, 'below');
					menu?.close();
				}}
			/>
		</ContextMenuSection>
	{/if}
</ContextMenu>
