<script lang="ts" module>
	type CommitContextData = {
		commitId: string;
		commitMessage: string;
		commitUrl: string | undefined;
		commitStatus: CommitStatusType;
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
					onUncommitClick: (event: MouseEvent) => void;
					onEditMessageClick: (event: MouseEvent) => void;
					onPatchEditClick: (event: MouseEvent) => void;
			  }
			| { commitStatus: 'Remote' | 'Base' | 'Integrated' }
		);

	export type CommitMenuContext = {
		position: { coords?: { x: number; y: number }; element?: HTMLElement };
		data: CommitContextData;
	};
</script>

<script lang="ts">
	import ContextMenu from '$components/ContextMenu.svelte';
	import { writeClipboard } from '$lib/backend/clipboard';
	import { rewrapCommitMessage } from '$lib/config/uiFeatureFlags';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import type { CommitStatusType } from '$lib/commits/commit';

	type Props = {
		projectId: string;
		openId?: string;
		context?: CommitMenuContext;
	};

	let { projectId, context = $bindable(), openId = $bindable() }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const [insertBlankCommitInBranch, commitInsertion] = stackService.insertBlankCommit;

	async function insertBlankCommit(commitId: string, location: 'above' | 'below' = 'below') {
		if (!context) return;
		if (
			context?.data.commitStatus !== 'LocalOnly' &&
			context?.data.commitStatus !== 'LocalAndRemote'
		) {
			return;
		}
		await insertBlankCommitInBranch({
			projectId,
			stackId: context?.data.stackId,
			commitId: commitId,
			offset: location === 'above' ? -1 : 1
		});
	}

	function close() {
		context = undefined;
	}
</script>

{#if context?.data}
	{@const { commitId, commitUrl, commitMessage } = context.data}
	<ContextMenu
		position={context.position}
		onclose={() => (context = undefined)}
		testId={TestId.CommitRowContextMenu}
	>
		{#if context.data.commitStatus === 'LocalAndRemote' || context.data.commitStatus === 'LocalOnly'}
			{@const { onUncommitClick, onEditMessageClick, onPatchEditClick } = context.data}
			<ContextMenuSection>
				<ContextMenuItem
					label="Uncommit"
					testId={TestId.CommitRowContextMenu_UncommitMenuButton}
					onclick={(e: MouseEvent) => {
						onUncommitClick?.(e);
						close();
					}}
				/>
				<ContextMenuItem
					label="Edit commit message"
					testId={TestId.CommitRowContextMenu_EditMessageMenuButton}
					onclick={(e: MouseEvent) => {
						onEditMessageClick?.(e);
						close();
					}}
				/>
				<ContextMenuItem
					label="Edit commit"
					onclick={(e: MouseEvent) => {
						onPatchEditClick?.(e);
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
				label="Copy commit hash"
				onclick={() => {
					writeClipboard(commitId);
					close();
				}}
			/>
			<ContextMenuItem
				label="Copy commit message"
				onclick={() => {
					writeClipboard(commitMessage);
					close();
				}}
			/>
		</ContextMenuSection>
		{#if context.data.commitStatus === 'LocalAndRemote' || context.data.commitStatus === 'LocalOnly'}
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
		<ContextMenuSection>
			<ContextMenuItem
				label={$rewrapCommitMessage ? 'Show original wrapping' : 'Rewrap message'}
				disabled={commitInsertion.current.isLoading}
				onclick={() => {
					rewrapCommitMessage.set(!$rewrapCommitMessage);
					close();
				}}
			/>
		</ContextMenuSection>
	</ContextMenu>
{/if}
