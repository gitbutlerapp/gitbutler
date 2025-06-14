<script lang="ts">
	import Resizer from '$components/Resizer.svelte';
	import AsyncRender from '$components/v3/AsyncRender.svelte';
	import BranchList from '$components/v3/BranchList.svelte';
	import BranchView from '$components/v3/BranchView.svelte';
	import CommitView from '$components/v3/CommitView.svelte';
	import NewCommitView from '$components/v3/NewCommitView.svelte';
	import SelectionView from '$components/v3/SelectionView.svelte';
	import WorktreeChanges from '$components/v3/WorktreeChanges.svelte';
	import { isParsedError } from '$lib/error/parser';
	import { DefinedFocusable } from '$lib/focus/focusManager.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { readKey } from '$lib/selection/key';
	import { UncommittedService } from '$lib/selection/uncommittedService.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { intersectionObserver } from '@gitbutler/ui/utils/intersectionObserver';
	import type { Stack } from '$lib/stacks/stack';

	type Props = {
		projectId: string;
		stack: Stack;
		focusedStackId?: string;
		onVisible: (visible: boolean) => void;
		clientWidth?: number;
		clientHeight?: number;
	};

	let {
		projectId,
		stack,
		focusedStackId,
		clientHeight = $bindable(),
		clientWidth = $bindable(),
		onVisible
	}: Props = $props();

	let lanesSrollableEl = $state<HTMLDivElement>();

	const [uiState, uncommittedService, idSelection, stackService] = inject(
		UiState,
		UncommittedService,
		IdSelection,
		StackService
	);
	const projectState = $derived(uiState.project(projectId));
	const exclusiveAction = $derived(projectState.exclusiveAction.current);
	const isCommitting = $derived(
		exclusiveAction?.type === 'commit' && exclusiveAction.stackId === stack.id
	);

	let dropzoneActivated = $state(false);

	const stackState = uiState.stack(stack.id);
	const selection = $derived(stackState.selection);
	const assignedSelection = idSelection.getById({
		type: 'worktree',
		stackId: stack.id
	});
	const lastAddedAssigned = assignedSelection.lastAdded;
	const assignedKey = $derived(
		$lastAddedAssigned?.key ? readKey($lastAddedAssigned.key) : undefined
	);

	const commitId = $derived(selection.current?.commitId);
	const branchName = $derived(selection.current?.branchName);
	const upstream = $derived(selection.current?.upstream);

	const selectedLastAdded = $derived.by(() => {
		if (commitId) {
			return idSelection.getById({ type: 'commit', commitId }).lastAdded;
		} else if (branchName) {
			return idSelection.getById({ type: 'branch', stackId: stack.id, branchName }).lastAdded;
		}
	});
	const selectedKey = $derived(
		$selectedLastAdded?.key ? readKey($selectedLastAdded.key) : undefined
	);
	const changes = uncommittedService.changesByStackId(stack.id || null);

	let laneEl = $state<HTMLDivElement>();
	let detailsEl = $state<HTMLDivElement>();
	let previewEl = $state<HTMLDivElement>();

	const defaultBranchResult = $derived(stackService.defaultBranch(projectId, stack.id));
	const defaultBranchName = $derived(defaultBranchResult?.current.data);

	function startCommit() {
		projectState.exclusiveAction.set({
			type: 'commit',
			branchName: defaultBranchName,
			stackId: stack.id
		});
		uncommittedService.checkAll(stack.id || null);
		uncommittedService.checkAll(null);
	}

	function onclose() {
		selection.set(undefined);
	}
</script>

<AsyncRender>
	<div
		class="lane"
		data-id={stack.id}
		bind:clientWidth
		bind:clientHeight
		bind:this={laneEl}
		data-testid={TestId.Stack}
		data-testid-stackid={stack.id}
		data-testid-stack={stack.heads.at(0)?.name}
		style:width={uiState.global.stackWidth.current + 'rem'}
		use:intersectionObserver={{
			callback: (entry) => {
				onVisible(!!entry?.isIntersecting);
			},
			options: {
				threshold: 0.5,
				root: lanesSrollableEl
			}
		}}
		use:focusable={{
			id: DefinedFocusable.Stack + ':' + stack.id,
			parentId: DefinedFocusable.ViewportRight
		}}
	>
		<div
			class="assignments"
			class:assignments__empty={changes.current.length === 0}
			class:committing={isCommitting}
			class:dropzone-activated={dropzoneActivated && changes.current.length === 0}
		>
			<WorktreeChanges
				title="Assigned"
				{projectId}
				stackId={stack.id}
				mode="assigned"
				active={focusedStackId === stack.id}
				dropzoneVisible={changes.current.length === 0 && !isCommitting}
				onDropzoneActivated={(activated) => {
					dropzoneActivated = activated;
				}}
			>
				{#snippet emptyPlaceholder()}
					<div class="assigned-changes-empty">
						<p class="text-12 text-body assigned-changes-empty__text">
							Drop files to assign to the lane
						</p>
					</div>
				{/snippet}
			</WorktreeChanges>
		</div>
		<div class="new-commit">
			{#if !isCommitting}
				<div class="start-commit">
					<Button
						testId={TestId.StartCommitButton}
						type="button"
						wide
						disabled={defaultBranchResult?.current.isLoading}
						onclick={() => {
							startCommit();
						}}
					>
						Start a commit…
					</Button>
				</div>
			{:else if isCommitting}
				<div class="message-editor">
					<NewCommitView {projectId} stackId={stack.id} noDrawer />
				</div>
			{/if}
		</div>
		<BranchList {projectId} stackId={stack.id} {focusedStackId}>
			{#snippet assignments()}{/snippet}
		</BranchList>
		<Resizer
			viewport={laneEl}
			direction="right"
			minWidth={16}
			maxWidth={64}
			onWidth={(value) => uiState.global.stackWidth.set(value)}
		/>
	</div>
	<!-- eslint-disable-next-line func-style -->
	{@const onerror = (err: unknown) => {
		// Clear selection if branch not found.
		if (isParsedError(err) && err.code === 'errors.branch.notfound') {
			selection?.set(undefined);
			console.warn('Workspace selection cleared');
		}
	}}
	{#if assignedKey || branchName || commitId}
		<div
			bind:this={detailsEl}
			style:width={uiState.global.detailsWidth.current + 'rem'}
			class="details"
			use:focusable={{
				id: DefinedFocusable.UncommittedChanges + ':' + stack.id,
				parentId: DefinedFocusable.ViewportRight
			}}
			data-details={stack.id}
		>
			{#if assignedKey && assignedKey.type === 'worktree'}
				<SelectionView
					{projectId}
					selectionId={{ ...assignedKey, type: 'worktree', stackId: assignedKey.stackId }}
				/>
			{:else if branchName && commitId}
				<CommitView
					{projectId}
					stackId={stack.id}
					commitKey={{
						stackId: stack.id,
						branchName,
						commitId,
						upstream: !!upstream
					}}
					active={selectedKey?.type === 'commit' && focusedStackId === stack.id}
					{onerror}
					{onclose}
				/>
			{:else if branchName}
				<BranchView
					stackId={stack.id}
					{projectId}
					{branchName}
					active={selectedKey?.type === 'branch' &&
						selectedKey.branchName === branchName &&
						focusedStackId === stack.id}
					draggableFiles
					{onerror}
					{onclose}
				/>
			{/if}
			<Resizer
				viewport={detailsEl}
				direction="right"
				minWidth={16}
				maxWidth={56}
				onWidth={(value) => uiState.global.detailsWidth.set(value)}
			/>
		</div>
	{/if}
	{#if selectedKey && !assignedKey}
		<div
			bind:this={previewEl}
			style:width={uiState.global.previewWidth.current + 'rem'}
			class="preview"
			use:focusable={{
				id: DefinedFocusable.Stack + ':' + stack.id,
				parentId: DefinedFocusable.ViewportRight
			}}
		>
			<SelectionView {projectId} selectionId={selectedKey} />
			<Resizer
				viewport={previewEl}
				direction="right"
				minWidth={20}
				maxWidth={96}
				onWidth={(value) => uiState.global.previewWidth.set(value)}
			/>
		</div>
	{/if}
</AsyncRender>

<style lang="postcss">
	.lane {
		display: flex;
		position: relative;
		flex-shrink: 0;
		flex-direction: column;
		width: 100%;
		max-width: unset;
		overflow-x: hidden;
		overflow-y: auto;
		border-right: 1px solid var(--clr-border-2);
		scroll-snap-align: start;

		&:first-child {
			border-left: 1px solid var(--clr-border-2);
		}
	}

	.assigned-changes-empty__text {
		width: 100%;
		color: var(--clr-text-2);
		text-align: center;
		opacity: 0.7;
		transition:
			color var(--transition-fast),
			opacity var(--transition-fast);
	}

	.assignments {
		display: flex;
		flex-direction: column;
		margin-bottom: 8px;
		margin: 12px 12px 0 12px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-bottom: none;
		border-radius: var(--radius-ml) var(--radius-ml) 0 0;
		/* background-color: var(--clr-bg-1); */

		&.dropzone-activated {
			& .assigned-changes-empty {
				padding: 14px 8px 14px;
				background-color: var(--clr-bg-1);
				will-change: padding;
			}

			& .assigned-changes-empty__text {
				color: var(--clr-theme-pop-on-soft);
			}
		}
	}

	.assignments__empty {
		margin-top: 0;
		border-top: none;
		border-bottom: none;
		border-top-right-radius: 0;
		border-top-left-radius: 0;
		border-radius: 0;
	}

	.details,
	.preview {
		position: relative;
		flex-shrink: 0;
		border-right: 1px solid var(--clr-border-2);
		white-space: wrap;
	}

	/* EMPTY ASSIGN AREA */
	.assigned-changes-empty {
		display: flex;
		position: relative;
		padding: 6px 8px 8px;
		overflow: hidden;
		gap: 12px;
		background-color: var(--clr-bg-2);
		transition: background-color var(--transition-fast);
	}

	.new-commit {
		margin: 0 12px 0 12px;
		padding: 12px;
		border: 1px solid var(--clr-border-2);
		border-radius: 0 0 var(--radius-ml) var(--radius-ml);
		background-color: var(--clr-bg-1);
	}
</style>
