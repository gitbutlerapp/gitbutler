<script lang="ts">
	import { writeClipboard } from '$lib/backend/clipboard';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { getContext } from '@gitbutler/shared/context';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import type { CommitStatusType } from '$lib/commits/commit';

	type Props = {
		commitStatus: CommitStatusType;
		projectId: string;
		commitId: string;
		commitMessage: string;
		commitUrl: string | undefined;
		onPatchEditClick: (event: MouseEvent) => void;
		onToggle?: (isOpen: boolean, isLeftClick: boolean) => void;
		close: () => void;
	} & (
		| {
				commitStatus: 'LocalOnly' | 'LocalAndRemote' | 'Integrated' | 'Remote';
				stackId: string;
		  }
		| { commitStatus: 'Base' }
	) &
		(
			| {
					commitStatus: 'LocalOnly' | 'LocalAndRemote';
					onUncommitClick?: (event: MouseEvent) => void;
					onEditMessageClick?: (event: MouseEvent) => void;
					onPatchEditClick?: (event: MouseEvent) => void;
			  }
			| { commitStatus: 'Remote' | 'Base' | 'Integrated' }
		);

	let {
		projectId,
		menu = $bindable(),
		leftClickTrigger,
		rightClickTrigger,
		baseBranch,
		stackId,
		commit,
		commitUrl,
		onUncommitClick,
		onEditMessageClick,
		onPatchEditClick,
		onToggle
	}: Props = $props();

	const stackService = getContext(StackService);
	const [insertBlankCommitInBranch, commitInsertion] = stackService.insertBlankCommit;

	async function insertBlankCommit(commitId: string, location: 'above' | 'below' = 'below') {
		if (args.commitStatus !== 'LocalOnly' && args.commitStatus !== 'LocalAndRemote') {
			return;
		}
		await insertBlankCommitInBranch({
			projectId,
			stackId: args.stackId,
			commitOid: commitId,
			offset: location === 'above' ? -1 : 1
		});
	}
</script>

<ContextMenu bind:this={menu} {leftClickTrigger} {rightClickTrigger} ontoggle={onToggle}>
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
	<ContextMenuItem
		label="Copy commit message"
		onclick={() => {
			writeClipboard(commitMessage);
			close();
		}}
	/>
</ContextMenuSection>
{#if args.commitStatus === 'LocalAndRemote' || args.commitStatus === 'LocalOnly'}
	<ContextMenuSection>
		<ContextMenuItem
			label="Add empty commit above"
			disabled={commitInsertion.current.isLoading}
			onclick={() => {
				insertBlankCommit(commitId, 'above');
				close();
			}}
		/>
		<ContextMenuItem
			label="Add empty commit below"
			disabled={commitInsertion.current.isLoading}
			onclick={() => {
				insertBlankCommit(commitId, 'below');
				close();
			}}
		/>
	</ContextMenuSection>
{/if}
