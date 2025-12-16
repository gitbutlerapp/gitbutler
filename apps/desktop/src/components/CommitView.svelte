<script lang="ts">
	import CommitContextMenu from '$components/CommitContextMenu.svelte';
	import CommitDetails from '$components/CommitDetails.svelte';
	import CommitMessageEditor from '$components/CommitMessageEditor.svelte';
	import CommitTitle from '$components/CommitTitle.svelte';
	import Drawer from '$components/Drawer.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import { isLocalAndRemoteCommit } from '$components/lib';
	import { type CommitKey } from '$lib/commits/commit';
	import { rewrapCommitMessage } from '$lib/config/uiFeatureFlags';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { MODE_SERVICE } from '$lib/mode/modeService';
	import { showToast } from '$lib/notifications/toasts';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { splitMessage } from '$lib/utils/commitMessage';
	import { ensureValue } from '$lib/utils/validation';
	import { inject, injectOptional } from '@gitbutler/core/context';
	import { Button, TestId } from '@gitbutler/ui';

	import type { ComponentProps } from 'svelte';

	type Props = {
		projectId: string;
		stackId?: string;
		laneId: string;
		commitKey: CommitKey;
		active?: boolean;
		draggableFiles: boolean;
		grow?: boolean;
		clientHeight?: number;
		resizer?: Partial<ComponentProps<typeof Resizer>>;
		ontoggle?: (collapsed: boolean) => void;
		onerror: (err: unknown) => void;
		onclose?: () => void;
	};

	let {
		projectId,
		stackId,
		laneId,
		commitKey,
		grow,
		clientHeight = $bindable(),
		resizer,
		ontoggle,
		onerror,
		onclose
	}: Props & { isInEditMessageMode?: boolean } = $props();

	const stackService = inject(STACK_SERVICE);
	const uiState = inject(UI_STATE);

	// Component is read-only when stackId is undefined
	const isReadOnly = $derived(!stackId);

	const forge = inject(DEFAULT_FORGE_FACTORY);
	const modeService = injectOptional(MODE_SERVICE, undefined);
	const laneState = $derived(uiState.lane(laneId));
	const projectState = $derived(uiState.project(projectId));
	const selected = $derived(laneState.selection);
	const branchName = $derived(selected.current?.branchName);

	const commitQuery = $derived(
		stackService.commitById(projectId, commitKey.stackId, commitKey.commitId)
	);

	const [updateCommitMessage, messageUpdateQuery] = stackService.updateCommitMessage;

	type Mode = 'view' | 'edit';

	function setMode(newMode: Mode) {
		switch (newMode) {
			case 'edit':
				projectState.exclusiveAction.set({
					type: 'edit-commit-message',
					stackId,
					branchName: commitKey.branchName,
					commitId: commitKey.commitId
				});
				break;
			case 'view':
				projectState.exclusiveAction.set(undefined);
				break;
		}
	}

	const parsedMessage = $derived(
		commitQuery.response ? splitMessage(commitQuery.response.message) : undefined
	);

	function combineParts(title?: string, description?: string): string {
		if (!title) {
			return '';
		}
		if (description) {
			return `${title}\n\n${description}`;
		}
		return title;
	}

	async function saveCommitMessage(title: string, description: string) {
		const commitMessage = combineParts(title, description);
		if (!branchName) {
			throw new Error('No branch selected!');
		}
		if (!commitMessage) {
			showToast({ message: 'Commit message is required', style: 'danger' });
			return;
		}

		const newCommitId = await updateCommitMessage({
			projectId,
			stackId: ensureValue(stackId),
			commitId: commitKey.commitId,
			message: commitMessage
		});

		uiState
			.lane(ensureValue(stackId))
			.selection.set({ branchName, commitId: newCommitId, previewOpen: true });
		setMode('view');
	}

	async function handleUncommit() {
		if (!branchName) return;
		await stackService.uncommit({
			projectId,
			stackId: ensureValue(stackId),
			branchName,
			commitId: commitKey.commitId
		});
	}

	function canEdit() {
		return modeService !== undefined && !isReadOnly;
	}

	function cancelEdit() {
		setMode('view');
	}
</script>

<ReduxResult {stackId} {projectId} result={commitQuery.result} {onerror}>
	{#snippet children(commit, env)}
		<Drawer
			bind:clientHeight
			testId={TestId.CommitDrawer}
			persistId="commit-view-drawer-{projectId}-{stackId}-{commitKey.commitId}"
			{resizer}
			{grow}
			{ontoggle}
			onclose={() => {
				// When the commit view is closed, we also want to unset the
				// relevant uiState so things like commit buttons are usable.

				// TODO: We should consider having a modal to confirm the cancel
				cancelEdit();
				onclose?.();
			}}
			noshrink
		>
			{#snippet header()}
				<CommitTitle
					truncate
					commitMessage={commit.message}
					className="text-14 text-semibold text-body"
					editable={!isReadOnly}
				/>
			{/snippet}

			{#snippet actions(header)}
				{#if canEdit()}
					<Button
						testId={TestId.CommitDrawerActionEditMessage}
						size="tag"
						kind="ghost"
						icon="edit"
						onclick={() => setMode('edit')}
						tooltip={isReadOnly ? 'Read-only mode' : 'Edit commit message'}
						disabled={isReadOnly}
					/>
				{/if}
				{@const data = isLocalAndRemoteCommit(commit)
					? {
							stackId,
							commitId: commit.id,
							commitMessage: commit.message,
							commitStatus: commit.state.type,
							commitUrl: forge.current.commitUrl(commit.id),
							onUncommitClick: () => handleUncommit(),
							onEditMessageClick: () => setMode('edit')
						}
					: undefined}
				{#if data}
					<CommitContextMenu {projectId} rightClickTrigger={header} contextData={data} />
				{/if}
			{/snippet}

			<div class="commit-view">
				{#if projectState.exclusiveAction.current?.type === 'edit-commit-message' && projectState.exclusiveAction.current.commitId === commit.id}
					<div
						class="edit-commit-view"
						data-testid={TestId.EditCommitMessageBox}
						class:no-paddings={uiState.global.useFloatingBox.current}
					>
						<CommitMessageEditor
							noPadding
							projectId={env.projectId}
							stackId={env.stackId}
							action={({ title, description }) => saveCommitMessage(title, description)}
							actionLabel="Save changes"
							onCancel={cancelEdit}
							floatingBoxHeader="Edit commit message"
							loading={messageUpdateQuery.current.isLoading}
							existingCommitId={commit.id}
							title={parsedMessage?.title || ''}
							description={parsedMessage?.description || ''}
						/>
					</div>
				{:else}
					<CommitDetails {commit} rewrap={$rewrapCommitMessage} />
				{/if}
			</div>
		</Drawer>
	{/snippet}
</ReduxResult>

<style>
	.commit-view {
		position: relative;
		/* Limit the commit view to at most 40vh to ensure other sections remain visible */
		max-height: 50vh;
		background-color: var(--clr-bg-1);
	}

	.edit-commit-view {
		display: flex;
		flex-direction: column;
		padding: 14px;

		&.no-paddings {
			margin: -14px;
		}
	}
</style>
