<script lang="ts">
	import CommitContextMenu, { type CommitMenuContext } from '$components/CommitContextMenu.svelte';
	import CommitDetails from '$components/CommitDetails.svelte';
	import CommitMessageEditor from '$components/CommitMessageEditor.svelte';
	import CommitTitle from '$components/CommitTitle.svelte';
	import Drawer from '$components/Drawer.svelte';
	import KebabButton from '$components/KebabButton.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import { isLocalAndRemoteCommit } from '$components/lib';
	import { isCommit } from '$lib/branches/v3';
	import { type CommitKey } from '$lib/commits/commit';
	import { rewrapCommitMessage } from '$lib/config/uiFeatureFlags';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { MODE_SERVICE } from '$lib/mode/modeService';
	import { showToast } from '$lib/notifications/toasts';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { splitMessage } from '$lib/utils/commitMessage';
	import { inject, injectOptional } from '@gitbutler/shared/context';
	import { AsyncButton, Button } from '@gitbutler/ui';

	import type { TargetType } from '$lib/intelligentScrolling/service';
	import type { ComponentProps } from 'svelte';

	type Props = {
		projectId: string;
		stackId: string;
		commitKey: CommitKey;
		active?: boolean;
		draggableFiles: boolean;
		scrollToType?: TargetType;
		scrollToId?: string;
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
		commitKey,
		scrollToId,
		scrollToType,
		grow,
		clientHeight = $bindable(),
		resizer,
		ontoggle,
		onerror,
		onclose
	}: Props & { isInEditMessageMode?: boolean } = $props();

	const stackService = inject(STACK_SERVICE);
	const uiState = inject(UI_STATE);

	const forge = inject(DEFAULT_FORGE_FACTORY);
	const modeService = injectOptional(MODE_SERVICE, undefined);
	const stackState = $derived(uiState.stack(stackId));
	const projectState = $derived(uiState.project(projectId));
	const selected = $derived(stackState.selection.get());
	const branchName = $derived(selected.current?.branchName);

	const commitResult = $derived(
		commitKey.upstream
			? stackService.upstreamCommitById(projectId, commitKey)
			: stackService.commitById(projectId, commitKey)
	);

	const [updateCommitMessage, messageUpdateResult] = stackService.updateCommitMessage;

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
		commitResult.current.data ? splitMessage(commitResult.current.data.message) : undefined
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
			showToast({ message: 'Commit message is required', style: 'error' });
			return;
		}

		const newCommitId = await updateCommitMessage({
			projectId,
			stackId,
			commitId: commitKey.commitId,
			message: commitMessage
		});

		uiState.stack(stackId).selection.set({ branchName, commitId: newCommitId });
		setMode('view');
	}

	let commitMenuContext = $state<CommitMenuContext>();

	async function handleUncommit() {
		if (!branchName) return;
		await stackService.uncommit({ projectId, stackId, branchName, commitId: commitKey.commitId });
	}

	function canEdit() {
		return modeService !== undefined;
	}

	async function editPatch() {
		if (!canEdit()) return;
		await modeService!.enterEditMode({ commitId: commitKey.commitId, stackId, projectId });
	}

	function cancelEdit() {
		setMode('view');
	}
</script>

<ReduxResult {stackId} {projectId} result={commitResult.current} {onerror}>
	{#snippet children(commit, env)}
		{@const isConflicted = isCommit(commit) && commit.hasConflicts}

		<Drawer
			bind:clientHeight
			testId={TestId.CommitDrawer}
			{scrollToId}
			{scrollToType}
			{resizer}
			{grow}
			{ontoggle}
			{onclose}
			bottomBorder
			noshrink
		>
			{#snippet header()}
				<CommitTitle
					truncate
					commitMessage={commit.message}
					className="text-14 text-semibold text-body"
				/>
			{/snippet}

			{#snippet extraActions()}
				{#if isConflicted}
					<AsyncButton
						size="tag"
						kind="solid"
						style="error"
						action={editPatch}
						icon="warning-small"
						tooltip="Resolve conflicts"
					>
						Resolve
					</AsyncButton>
				{/if}

				{#if canEdit()}
					<Button
						testId={TestId.CommitDrawerActionEditMessage}
						size="tag"
						kind="ghost"
						icon="edit-text"
						onclick={() => setMode('edit')}
						tooltip="Edit commit message"
					/>
				{/if}
			{/snippet}

			{#snippet kebabMenu(header)}
				{@const data = isLocalAndRemoteCommit(commit)
					? {
							stackId,
							commitId: commit.id,
							commitMessage: commit.message,
							commitStatus: commit.state.type,
							commitUrl: forge.current.commitUrl(commit.id),
							onUncommitClick: () => handleUncommit(),
							onEditMessageClick: () => setMode('edit'),
							onPatchEditClick: () => editPatch()
						}
					: undefined}
				{#if data}
					<KebabButton
						contextElement={header}
						onclick={(element) => (commitMenuContext = { data, position: { element } })}
						oncontext={(coords) => (commitMenuContext = { data, position: { coords } })}
						activated={!!commitMenuContext?.position.element}
					/>
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
							loading={messageUpdateResult.current.isLoading}
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

{#if commitMenuContext}
	<CommitContextMenu {projectId} bind:context={commitMenuContext} />
{/if}

<style>
	.commit-view {
		position: relative;
		padding: 14px;
		background-color: var(--clr-bg-1);
	}

	.edit-commit-view {
		display: flex;
		flex-direction: column;

		&.no-paddings {
			margin: -14px;
		}
	}
</style>
