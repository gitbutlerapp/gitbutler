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
	import { inject } from '@gitbutler/core/context';
	import {
		ContextMenu,
		ContextMenuItem,
		ContextMenuItemSubmenu,
		ContextMenuSection,
		KebabButton,
		TestId
	} from '@gitbutler/ui';
	import type { AnchorPosition } from '$lib/stacks/stack';

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
	const [createRef, refCreation] = stackService.createReference;

	// Component is read-only when stackId is undefined
	const isReadOnly = $derived(
		contextData?.commitStatus === 'LocalAndRemote' || contextData?.commitStatus === 'LocalOnly'
			? !contextData.stackId
			: false
	);

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

	async function handleCreateNewRef(stackId: string, commitId: string, position: AnchorPosition) {
		const newName = await stackService.fetchNewBranchName(projectId);
		await createRef({
			projectId,
			stackId,
			request: {
				newName,
				anchor: {
					type: 'atCommit',
					subject: {
						commit_id: commitId,
						position
					}
				}
			}
		});
	}

	function closeContextMenu() {
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
						icon="undo-small"
						testId={TestId.CommitRowContextMenu_UncommitMenuButton}
						disabled={isReadOnly}
						onclick={(e: MouseEvent) => {
							if (!isReadOnly) {
								onUncommitClick?.(e);
								closeContextMenu();
							}
						}}
					/>
					<ContextMenuItem
						label="Edit commit message"
						icon="edit"
						testId={TestId.CommitRowContextMenu_EditMessageMenuButton}
						disabled={isReadOnly}
						onclick={(e: MouseEvent) => {
							if (!isReadOnly) {
								onEditMessageClick?.(e);
								closeContextMenu();
							}
						}}
					/>
					<ContextMenuItem
						label="Edit commit"
						icon="edit-commit"
						disabled={isReadOnly}
						onclick={(e: MouseEvent) => {
							if (!isReadOnly) {
								onPatchEditClick?.(e);
								closeContextMenu();
							}
						}}
					/>
				</ContextMenuSection>
			{/if}
			<ContextMenuSection>
				{#if commitUrl}
					<ContextMenuItem
						label="Open in browser"
						icon="open-link"
						onclick={async () => {
							await urlService.openExternalUrl(commitUrl);
							closeContextMenu();
						}}
					/>
				{/if}
				<ContextMenuItemSubmenu label="Copy" icon="copy">
					{#snippet submenu({ close })}
						<ContextMenuSection>
							{#if commitUrl}
								<ContextMenuItem
									label="Copy commit link"
									onclick={() => {
										clipboardService.write(commitUrl, { message: 'Commit link copied' });
										close();
										closeContextMenu();
									}}
								/>
							{/if}
							<ContextMenuItem
								label="Copy commit hash"
								onclick={() => {
									clipboardService.write(commitId, { message: 'Commit hash copied' });
									close();
									closeContextMenu();
								}}
							/>
							<ContextMenuItem
								label="Copy commit message"
								onclick={() => {
									clipboardService.write(commitMessage, { message: 'Commit message copied' });
									close();
									closeContextMenu();
								}}
							/>
						</ContextMenuSection>
					{/snippet}
				</ContextMenuItemSubmenu>
				{#if contextData.commitStatus === 'LocalAndRemote' || contextData.commitStatus === 'LocalOnly'}
					{@const stackId = contextData.stackId}

					<ContextMenuItemSubmenu label="Add empty commit" icon="new-empty-commit">
						{#snippet submenu({ close })}
							<ContextMenuSection>
								<ContextMenuItem
									label="Add empty commit above"
									disabled={isReadOnly || commitInsertion.current.isLoading}
									onclick={() => {
										insertBlankCommit(ensureValue(stackId), commitId, 'above');
										close();
										closeContextMenu();
									}}
								/>
								<ContextMenuItem
									label="Add empty commit below"
									disabled={isReadOnly || commitInsertion.current.isLoading}
									onclick={() => {
										insertBlankCommit(ensureValue(stackId), commitId, 'below');
										close();
										closeContextMenu();
									}}
								/>
							</ContextMenuSection>
						{/snippet}
					</ContextMenuItemSubmenu>
					<ContextMenuItemSubmenu label="Create branch" icon="branch-remote">
						{#snippet submenu({ close })}
							<ContextMenuSection>
								<ContextMenuItem
									label="Branch from this commit"
									disabled={isReadOnly || refCreation.current.isLoading}
									onclick={async () => {
										if (!isReadOnly) {
											await handleCreateNewRef(ensureValue(stackId), commitId, 'Above');
											close();
											closeContextMenu();
										}
									}}
								/>
								<ContextMenuItem
									label="Branch after this commit"
									disabled={isReadOnly || refCreation.current.isLoading}
									onclick={async () => {
										if (!isReadOnly) {
											await handleCreateNewRef(ensureValue(stackId), commitId, 'Below');
											close();
											closeContextMenu();
										}
									}}
								/>
							</ContextMenuSection>
						{/snippet}
					</ContextMenuItemSubmenu>
				{/if}
			</ContextMenuSection>

			<ContextMenuSection>
				<ContextMenuItem
					label={$rewrapCommitMessage ? 'Show original wrapping' : 'Rewrap message'}
					icon="text-wrap"
					disabled={commitInsertion.current.isLoading}
					onclick={() => {
						rewrapCommitMessage.set(!$rewrapCommitMessage);
						closeContextMenu();
					}}
				/>
			</ContextMenuSection>
		{/if}
	</ContextMenu>
{/if}
