<script lang="ts">
	import { writeClipboard } from '$lib/backend/clipboard';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { BranchStack } from '$lib/branches/branch';
	import { type Commit, type DetailedCommit } from '$lib/commits/commit';
	import { Project } from '$lib/project/project';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { getContext } from '@gitbutler/shared/context';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';

	interface Props {
		menu: ReturnType<typeof ContextMenu> | undefined;
		leftClickTrigger: HTMLElement | undefined;
		rightClickTrigger: HTMLElement | undefined;
		baseBranch: BaseBranch;
		stack: BranchStack | undefined;
		commit: DetailedCommit | Commit;
		commitUrl: string | undefined;
		isRemote: boolean;
		onUncommitClick: (event: MouseEvent) => void;
		onEditMessageClick: (event: MouseEvent) => void;
		onClose?: () => void;
		onToggle?: (isOpen: boolean, isLeftClick: boolean) => void;
	}

	let {
		menu = $bindable(),
		leftClickTrigger,
		rightClickTrigger,
		baseBranch,
		stack: branch,
		commit,
		commitUrl,
		isRemote,
		onUncommitClick,
		onEditMessageClick,
		onClose,
		onToggle
	}: Props = $props();

	const project = getContext(Project);
	const stackService = getContext(StackService);
	const [insertBlankCommitMutation] = stackService.insertBlankCommit;

	async function insertBlankCommit(commitId: string, location: 'above' | 'below' = 'below') {
		if (!branch || !baseBranch) {
			console.error('Unable to insert commit');
			return;
		}
		await insertBlankCommitMutation({
			projectId: project.id,
			stackId: branch.id,
			commitOid: commitId,
			offset: location === 'above' ? -1 : 1
		});
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
					writeClipboard(commitUrl);
					menu?.close();
				}}
			/>
		{/if}
		<ContextMenuItem
			label="Copy commit message"
			onclick={() => {
				writeClipboard(commit.description);
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
