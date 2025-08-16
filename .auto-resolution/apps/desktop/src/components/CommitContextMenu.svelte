<script lang="ts" module>
	import type { CommitStatusType } from '$lib/commits/commit';
	interface BaseContextData {
		commitStatus: CommitStatusType;
		commitId: string;
		commitMessage: string;
		commitUrl?: string;
	}

	interface LocalCommitContextData extends BaseContextData {
		commitStatus: 'LocalOnly' | 'LocalAndRemote';
		stackId?: string;
		onUncommitClick: (event: MouseEvent) => void;
		onEditMessageClick: (event: MouseEvent) => void;
		onPatchEditClick: (event: MouseEvent) => void;
	}

	interface RemoteCommitContextData extends BaseContextData {
		commitStatus: 'Remote';
		stackId?: string;
	}

	interface IntegratedCommitContextData extends BaseContextData {
		commitStatus: 'Integrated';
		stackId?: string;
	}

	interface BaseCommitContextData extends BaseContextData {
		commitStatus: 'Base';
	}

	export type CommitContextData =
		| LocalCommitContextData
		| RemoteCommitContextData
		| IntegratedCommitContextData
		| BaseCommitContextData;

	export type CommitMenuContext = {
		position: { coords?: { x: number; y: number }; element?: HTMLElement };
		data: CommitContextData;
	};
</script>

<script lang="ts">
	import { CLIPBOARD_SERVICE } from '$lib/backend/clipboard';
	import { rewrapCommitMessage } from '$lib/config/uiFeatureFlags';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { URL_SERVICE } from '$lib/utils/url';
	import { ensureValue } from '$lib/utils/validation';
	import { inject } from '@gitbutler/shared/context';
	import {
		ContextMenu,
		ContextMenuItem,
		ContextMenuSection,
		KebabButton,
		TestId
	} from '@gitbutler/ui';

	type Props = {
		flat?: boolean;
		projectId: string;
		openId?: string;
		rightClickTrigger?: HTMLElement;
		contextData: CommitContextData | undefined;
	};

	let { flat, projectId, openId = $bindable(), rightClickTrigger, contextData }: Props = $props();

	const urlService = inject(URL_SERVICE);
	const stackService = inject(STACK_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);
	const [insertBlankCommitInBranch, commitInsertion] = stackService.insertBlankCommit;

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let kebabButtonElement = $state<HTMLElement>();

	async function insertBlankCommit(
		stackId: string,
		commitId: string,
		location: 'above' | 'below' = 'below'
	) {
		await insertBlankCommitInBranch({
			projectId,
			stackId,
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
							await urlService.openExternalUrl(commitUrl);
							close();
						}}
					/>
					<ContextMenuItem
						label="Copy commit link"
						onclick={() => {
							clipboardService.write(commitUrl);
							close();
						}}
					/>
				{/if}
				<ContextMenuItem
					label="Copy commit hash"
					onclick={() => {
						clipboardService.write(commitId);
						close();
					}}
				/>
				<ContextMenuItem
					label="Copy commit message"
					onclick={() => {
						clipboardService.write(commitMessage);
						close();
					}}
				/>
			</ContextMenuSection>
			{#if contextData.commitStatus === 'LocalAndRemote' || contextData.commitStatus === 'LocalOnly'}
				{@const stackId = contextData.stackId}
				<ContextMenuSection>
					<ContextMenuItem
						label="Add empty commit above"
						disabled={commitInsertion.current.isLoading}
						onclick={() => {
							insertBlankCommit(ensureValue(stackId), commitId, 'above');
							close();
						}}
					/>
					<ContextMenuItem
						label="Add empty commit below"
						disabled={commitInsertion.current.isLoading}
						onclick={() => {
							insertBlankCommit(ensureValue(stackId), commitId, 'below');
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
