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
	import { writeClipboard } from '$lib/backend/clipboard';
	import { rewrapCommitMessage } from '$lib/config/uiFeatureFlags';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import {
		ContextMenu,
		ContextMenuItem,
		ContextMenuSection,
		KebabButton,
		TestId
	} from '@gitbutler/ui';

	import type { CommitStatusType } from '$lib/commits/commit';

	type Props = {
		flat?: boolean;
		projectId: string;
		openId?: string;
		context?: CommitMenuContext;
		rightClickTrigger?: HTMLElement;
		contextData?: CommitContextData;
	};

	let {
		flat,
		projectId,
		context = $bindable(),
		openId = $bindable(),
		rightClickTrigger,
		contextData
	}: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const [insertBlankCommitInBranch, commitInsertion] = stackService.insertBlankCommit;

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let kebabButtonElement = $state<HTMLElement>();

	async function insertBlankCommit(commitId: string, location: 'above' | 'below' = 'below') {
		const data = context?.data ?? contextData;
		if (!data) return;
		if (data.commitStatus !== 'LocalOnly' && data.commitStatus !== 'LocalAndRemote') {
			return;
		}
		await insertBlankCommitInBranch({
			projectId,
			stackId: data.stackId,
			commitId: commitId,
			offset: location === 'above' ? -1 : 1
		});
	}

	function close() {
		contextMenu?.close();
	}
</script>

{#if rightClickTrigger && contextData}
	<KebabButton
		{flat}
		bind:el={kebabButtonElement}
		contextElement={rightClickTrigger}
		testId={TestId.KebabMenuButton}
		onclick={() => {
			contextMenu?.toggle();
		}}
		oncontext={(e) => {
			contextMenu?.open(e);
		}}
	/>

	<ContextMenu
		bind:this={contextMenu}
		leftClickTrigger={kebabButtonElement}
		{rightClickTrigger}
		testId={TestId.CommitRowContextMenu}
	>
		{#if contextData}
			{@const { commitId, commitUrl, commitMessage } = contextData}
			{#if contextData.commitStatus === 'LocalAndRemote' || contextData.commitStatus === 'LocalOnly'}
				{@const { onUncommitClick, onEditMessageClick, onPatchEditClick } = contextData}
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
			{#if contextData.commitStatus === 'LocalAndRemote' || contextData.commitStatus === 'LocalOnly'}
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
		{/if}
	</ContextMenu>
{/if}
