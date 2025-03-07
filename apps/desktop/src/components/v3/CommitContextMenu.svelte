<script lang="ts">
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { isCommit, type Commit, type UpstreamCommit } from '$lib/branches/v3';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { copyToClipboard } from '@gitbutler/shared/clipboard';
	import { getContext } from '@gitbutler/shared/context';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';

	interface Props {
		projectId: string;
		menu: ReturnType<typeof ContextMenu> | undefined;
		leftClickTrigger: HTMLElement | undefined;
		rightClickTrigger: HTMLElement | undefined;
		baseBranch: BaseBranch;
		branchId: string | undefined;
		commit: Commit | UpstreamCommit;
		commitUrl: string | undefined;
		onUncommitClick: (event: MouseEvent) => void;
		onEditMessageClick: (event: MouseEvent) => void;
		onPatchEditClick: (event: MouseEvent) => void;
		onClose?: () => void;
		onToggle?: (isOpen: boolean, isLeftClick: boolean) => void;
	}

	let {
		projectId,
		menu = $bindable(),
		leftClickTrigger,
		rightClickTrigger,
		baseBranch,
		branchId,
		commit,
		commitUrl,
		onUncommitClick,
		onEditMessageClick,
		onPatchEditClick,
		onClose,
		onToggle
	}: Props = $props();

	const stackService = getContext(StackService);

	function insertBlankCommit(commitId: string, location: 'above' | 'below' = 'below') {
		if (!branchId || !baseBranch) {
			console.error('Unable to insert commit', { branchId, baseBranch });
			return;
		}
		stackService.insertBlankCommit(projectId, branchId, commitId, location === 'above' ? -1 : 1);
	}

	const isRemote = $derived(!isCommit(commit));
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
			<!-- TODO: Re-enable the option once it works -->
			<ContextMenuItem
				label="Edit commit message"
				disabled
				onclick={(e: MouseEvent) => {
					onEditMessageClick(e);
					menu?.close();
				}}
			/>
			<!-- TODO: Re-enable the option once it works -->
			<ContextMenuItem
				label="Edit commit"
				disabled
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
				copyToClipboard(commit.message);
				menu?.close();
			}}
		/>
	</ContextMenuSection>
	{#if !isRemote}
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
