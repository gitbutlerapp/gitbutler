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

	let { projectId, commitId, commitMessage, commitUrl, close, ...args }: Props = $props();

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

{#if args.commitStatus === 'LocalAndRemote' || args.commitStatus === 'LocalOnly'}
	<ContextMenuSection>
		<ContextMenuItem
			label="Uncommit"
			onclick={(e: MouseEvent) => {
				args.onUncommitClick?.(e);
				close();
			}}
		/>
		<!-- TODO: Re-enable the option once it works -->
		<ContextMenuItem
			label="Edit commit message"
			disabled
			onclick={(e: MouseEvent) => {
				args.onEditMessageClick?.(e);
				close();
			}}
		/>
		<!-- TODO: Re-enable the option once it works -->
		<ContextMenuItem
			label="Edit commit"
			disabled
			onclick={(e: MouseEvent) => {
				args.onPatchEditClick?.(e);
				close();
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
				close();
			}}
		/>
		<ContextMenuItem
			label="Copy commit link"
			onclick={() => {
				writeClipboard(commitUrl);
				close();
			}}
		/>
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
