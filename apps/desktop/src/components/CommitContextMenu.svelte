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
	import { editPatch } from '$lib/editMode/editPatchUtils';
	import { MODE_SERVICE } from '$lib/mode/modeService';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { URL_SERVICE } from '$lib/utils/url';
	import { ensureValue } from '$lib/utils/validation';
	import { inject, injectOptional } from '@gitbutler/core/context';
	import {
		ContextMenuItem,
		ContextMenuItemSubmenu,
		ContextMenuSection,
		KebabButton,
		TestId
	} from '@gitbutler/ui';
	import type { AnchorPosition } from '$lib/stacks/stack';

	type Props = {
		showOnHover?: boolean;
		projectId: string;
		openId?: string;
		rightClickTrigger?: HTMLElement;
		contextData: CommitContextData | undefined;
	};

	let {
		showOnHover,
		projectId,
		openId = $bindable(),
		rightClickTrigger,
		contextData
	}: Props = $props();

	const urlService = inject(URL_SERVICE);
	const stackService = inject(STACK_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);
	const modeService = injectOptional(MODE_SERVICE, undefined);
	const [insertBlankCommitInBranch, commitInsertion] = stackService.insertBlankCommit;
	const [createRef, refCreation] = stackService.createReference;

	// Component is read-only when stackId is undefined
	const isReadOnly = $derived(
		contextData?.commitStatus === 'LocalAndRemote' || contextData?.commitStatus === 'LocalOnly'
			? !contextData.stackId
			: false
	);

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

	async function handleEditPatch(commitId: string, stackId: string) {
		if (isReadOnly) return;
		await editPatch({
			modeService,
			commitId,
			stackId,
			projectId
		});
	}
</script>

{#if rightClickTrigger && contextData}
	<KebabButton {showOnHover} contextElement={rightClickTrigger} testId={TestId.KebabMenuButton}>
		{#snippet contextMenu({ close })}
			{@const { commitId, commitUrl, commitMessage } = contextData}
			{#if contextData.commitStatus === 'LocalAndRemote' || contextData.commitStatus === 'LocalOnly'}
				{@const { onUncommitClick, onEditMessageClick } = contextData}
				<ContextMenuSection>
					<ContextMenuItem
						label="Uncommit"
						icon="undo-small"
						testId={TestId.CommitRowContextMenu_UncommitMenuButton}
						disabled={isReadOnly}
						onclick={(e: MouseEvent) => {
							if (!isReadOnly) {
								onUncommitClick?.(e);
								close();
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
								close();
							}
						}}
					/>
					<ContextMenuItem
						label="Edit commit"
						icon="edit-commit"
						testId={TestId.CommitRowContextMenu_EditCommit}
						disabled={isReadOnly}
						onclick={async () => {
							if (!isReadOnly && contextData.stackId) {
								await handleEditPatch(commitId, contextData.stackId);
								close();
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
							close();
						}}
					/>
				{/if}
				<ContextMenuItemSubmenu label="Copy" icon="copy">
					{#snippet submenu({ close: closeSubmenu })}
						<ContextMenuSection>
							{#if commitUrl}
								<ContextMenuItem
									label="Copy commit link"
									onclick={() => {
										clipboardService.write(commitUrl, { message: 'Commit link copied' });
										closeSubmenu();
										close();
									}}
								/>
							{/if}
							<ContextMenuItem
								label="Copy commit hash"
								onclick={() => {
									clipboardService.write(commitId, { message: 'Commit hash copied' });
									closeSubmenu();
									close();
								}}
							/>
							<ContextMenuItem
								label="Copy commit message"
								onclick={() => {
									clipboardService.write(commitMessage, { message: 'Commit message copied' });
									closeSubmenu();
									close();
								}}
							/>
						</ContextMenuSection>
					{/snippet}
				</ContextMenuItemSubmenu>
				{#if contextData.commitStatus === 'LocalAndRemote' || contextData.commitStatus === 'LocalOnly'}
					{@const stackId = contextData.stackId}

					<ContextMenuItemSubmenu label="Add empty commit" icon="new-empty-commit">
						{#snippet submenu({ close: closeSubmenu })}
							<ContextMenuSection>
								<ContextMenuItem
									label="Add empty commit above"
									disabled={isReadOnly || commitInsertion.current.isLoading}
									onclick={() => {
										insertBlankCommit(ensureValue(stackId), commitId, 'above');
										closeSubmenu();
										close();
									}}
								/>
								<ContextMenuItem
									label="Add empty commit below"
									disabled={isReadOnly || commitInsertion.current.isLoading}
									onclick={() => {
										insertBlankCommit(ensureValue(stackId), commitId, 'below');
										closeSubmenu();
										close();
									}}
								/>
							</ContextMenuSection>
						{/snippet}
					</ContextMenuItemSubmenu>
					<ContextMenuItemSubmenu label="Create branch" icon="branch-remote">
						{#snippet submenu({ close: closeSubmenu })}
							<ContextMenuSection>
								<ContextMenuItem
									label="Branch from this commit"
									disabled={isReadOnly || refCreation.current.isLoading}
									onclick={async () => {
										if (!isReadOnly) {
											await handleCreateNewRef(ensureValue(stackId), commitId, 'Above');
											closeSubmenu();
											close();
										}
									}}
								/>
								<ContextMenuItem
									label="Branch after this commit"
									disabled={isReadOnly || refCreation.current.isLoading}
									onclick={async () => {
										if (!isReadOnly) {
											await handleCreateNewRef(ensureValue(stackId), commitId, 'Below');
											closeSubmenu();
											close();
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
						close();
					}}
				/>
			</ContextMenuSection>
		{/snippet}
	</KebabButton>
{/if}
