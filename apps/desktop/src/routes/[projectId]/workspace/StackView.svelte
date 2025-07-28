<script lang="ts">
	import Changes from './Changes.svelte';
	import AsyncRender from '$components/shared/AsyncRender.svelte';
	import ChangedFiles from '$components/shared/ChangedFiles.svelte';
	import ConfigurableScrollableContainer from '$components/shared/ConfigurableScrollableContainer.svelte';
	import Drawer from '$components/shared/Drawer.svelte';
	import ReduxResult from '$components/shared/ReduxResult.svelte';
	import Resizer from '$components/shared/Resizer.svelte';
	import SelectionView from '$components/shared/SelectionView.svelte';
	import BranchList from '$components/shared/branches/BranchList.svelte';
	import BranchView from '$components/shared/branches/BranchView.svelte';
	import CommitView from '$components/shared/commits/CommitView.svelte';
	import NewCommitView from '$components/shared/commits/NewCommitView.svelte';
	import { isParsedError } from '$lib/error/parser';
	import { DefinedFocusable } from '$lib/focus/focusManager.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { DIFF_SERVICE } from '$lib/hunks/diffService.svelte';
	import {
		INTELLIGENT_SCROLLING_SERVICE,
		scrollingAttachment
	} from '$lib/intelligentScrolling/service';
	import { ID_SELECTION } from '$lib/selection/idSelection.svelte';
	import { readKey, type SelectionId } from '$lib/selection/key';
	import { UNCOMMITTED_SERVICE } from '$lib/selection/uncommittedService.svelte';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UI_STATE } from '$lib/state/uiState.svelte';

	import { TestId } from '$lib/testing/testIds';
	import { computeChangeStatus } from '$lib/utils/fileStatus';
	import { inject } from '@gitbutler/shared/context';
	import { persistWithExpiration } from '@gitbutler/shared/persisted';

	import { Button, FileViewHeader, Icon } from '@gitbutler/ui';
	import { intersectionObserver } from '@gitbutler/ui/utils/intersectionObserver';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
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

	const stackService = inject(STACK_SERVICE);
	const diffService = inject(DIFF_SERVICE);
	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const uiState = inject(UI_STATE);
	const intelligentScrollingService = inject(INTELLIGENT_SCROLLING_SERVICE);
	const idSelection = inject(ID_SELECTION);

	const projectState = $derived(uiState.project(projectId));

	const action = $derived(projectState.exclusiveAction.current);
	const isCommitting = $derived(action?.type === 'commit' && action.stackId === stack.id);

	const userSettings = inject(SETTINGS);
	const zoom = $derived($userSettings.zoom);

	// If the user is making a commit to a different lane we dim this one.
	const dimmed = $derived(action?.type === 'commit' && action?.stackId !== stack.id);

	const persistedStackWidth = persistWithExpiration(
		uiState.global.stackWidth.current,
		`ui-stack-width-${stack.id}`,
		1440
	);

	const branchesResult = $derived(stackService.branches(projectId, stack.id));

	let dropzoneActivated = $state(false);

	const stackState = $derived(uiState.stack(stack.id));
	const selection = $derived(stackState.selection);
	const assignedSelection = $derived(
		idSelection.getById({
			type: 'worktree',
			stackId: stack.id
		})
	);
	const lastAddedAssigned = $derived(assignedSelection.lastAdded);
	const assignedKey = $derived(
		$lastAddedAssigned?.key ? readKey($lastAddedAssigned.key) : undefined
	);

	const commitId = $derived(selection.current?.commitId);
	const branchName = $derived(selection.current?.branchName);
	const upstream = $derived(selection.current?.upstream);

	const activeSelectionId: SelectionId | undefined = $derived.by(() => {
		if (commitId) {
			return { type: 'commit', commitId, stackId: stack.id };
		} else if (branchName) {
			return { type: 'branch', stackId: stack.id, branchName };
		}
	});

	const activeLastAdded = $derived.by(() => {
		if (activeSelectionId) {
			return idSelection.getById(activeSelectionId).lastAdded;
		}
	});

	const selectedFile = $derived($activeLastAdded?.key ? readKey($activeLastAdded.key) : undefined);

	const previewKey = $derived(assignedKey || selectedFile);
	const previewChangeResult = $derived(
		previewKey ? idSelection.changeByKey(projectId, previewKey) : undefined
	);

	const changes = $derived(uncommittedService.changesByStackId(stack.id || null));

	const unsetMaxHeight = '25%';

	let stackViewEl = $state<HTMLDivElement>();
	let compactDiv = $state<HTMLDivElement>();

	let verticalHeight = $state<number>(0);
	let verticalHeightRem = $derived(pxToRem(verticalHeight, zoom));

	let actualDetailsHeight = $state<number>(0);
	let actualDetailsHeightRem = $derived(pxToRem(actualDetailsHeight, zoom));

	let minChangedFilesHeight = $state(8);
	let minPreviewHeight = $derived(previewChangeResult ? 7 : 0);

	let maxChangedFilesHeight = $derived(
		verticalHeightRem - actualDetailsHeightRem - minPreviewHeight
	);

	let changedFilesCollapsed = $state<boolean>();

	const defaultBranchResult = $derived(stackService.defaultBranch(projectId, stack.id));

	// Resizer configuration constants
	const RESIZER_CONFIG = {
		panel1: {
			minWidth: 18,
			maxWidth: 64,
			defaultValue: 23
		},
		panel2: {
			minWidth: 18,
			maxWidth: 56,
			defaultValue: 32
		}
	} as const;

	function startCommit(branchName: string) {
		projectState.exclusiveAction.set({
			type: 'commit',
			branchName,
			stackId: stack.id
		});
		const stackAssignments = uncommittedService.getAssignmentsByStackId(stack.id);
		if (stackAssignments.length > 0) {
			uncommittedService.checkAll(stack.id);
			uncommittedService.uncheckAll(null);
		} else {
			uncommittedService.checkAll(null);
		}
	}

	export function onclose() {
		selection.set(undefined);
		intelligentScrollingService.show(projectId, stack.id, 'stack');
	}

	const startCommitVisible = $derived(uncommittedService.startCommitVisible(stack.id));

	function onerror(err: unknown) {
		// Clear selection if branch not found.
		if (isParsedError(err) && err.code === 'errors.branch.notfound') {
			selection?.set(undefined);
			console.warn('Workspace selection cleared');
		}
	}
</script>

<!-- ATTENTION -->
<!--
	This file is growing complex, and should be simplified where possible.  It
	is also intentionally intriciate, as it allows us to render the components
	in two different configurations.
	
	While tedious to maintain, it is also a good forcing function for making
	components that compose better. Be careful when changing, especially since
	integration tests only covers the default layout.
-->

{#snippet assignedChangePreview(stackId?: string)}
	<SelectionView
		bottomBorder
		testId={TestId.WorktreeSelectionView}
		{projectId}
		selectionId={{ ...assignedKey, type: 'worktree', stackId }}
		onclose={() => {
			idSelection.clear({ type: 'worktree', stackId: stack.id });
			intelligentScrollingService.show(projectId, stack.id, 'stack');
		}}
		draggableFiles
	/>
{/snippet}

{#snippet otherChangePreview(selectionId: SelectionId)}
	<SelectionView
		testId={TestId.StackSelectionView}
		{projectId}
		{selectionId}
		diffOnly={true}
		onclose={() => {
			intelligentScrollingService.show(projectId, stack.id, 'details');
		}}
		draggableFiles={selectionId.type === 'commit'}
	/>
{/snippet}

{#snippet branchView(branchName: string)}
	<BranchView
		stackId={stack.id}
		{projectId}
		{branchName}
		active={selectedFile?.type === 'branch' &&
			selectedFile.branchName === branchName &&
			focusedStackId === stack.id}
		scrollToType="details"
		scrollToId={stack.id}
		bind:clientHeight={actualDetailsHeight}
		{onerror}
		{onclose}
	/>
{/snippet}

{#snippet commitView(branchName: string, commitId: string)}
	<CommitView
		{projectId}
		stackId={stack.id}
		commitKey={{
			stackId: stack.id,
			branchName,
			commitId,
			upstream: !!upstream
		}}
		draggableFiles
		active={selectedFile?.type === 'commit' && focusedStackId === stack.id}
		scrollToType="details"
		scrollToId={stack.id}
		bind:clientHeight={actualDetailsHeight}
		{onerror}
		{onclose}
	/>
{/snippet}

{#snippet commitChangedFiles(commitId: string)}
	{@const active = activeSelectionId?.type === 'commit' && focusedStackId === stack.id}
	{@const changesResult = stackService.commitChanges(projectId, commitId)}
	<ReduxResult {projectId} stackId={stack.id} result={changesResult.current}>
		{#snippet children(changes, { projectId, stackId })}
			<ChangedFiles
				title="Changed Files"
				{projectId}
				{stackId}
				draggableFiles
				selectionId={{ type: 'commit', commitId, stackId: stack.id }}
				noshrink={!!previewKey}
				ontoggle={(collapsed) => {
					changedFilesCollapsed = collapsed;
				}}
				changes={changes.changes.filter(
					(change) => !(change.path in (changes.conflictEntries?.entries ?? {}))
				)}
				conflictEntries={changes.conflictEntries}
				{active}
				resizer={{
					persistId: `resizer-panel2-changed-files-${stack.id}`,
					defaultValue: undefined,
					maxHeight: maxChangedFilesHeight,
					minHeight: minChangedFilesHeight,
					passive: !previewKey,
					order: 1,
					unsetMaxHeight: previewKey ? unsetMaxHeight : undefined
				}}
				autoselect
			/>
		{/snippet}
	</ReduxResult>
{/snippet}

{#snippet branchChangedFiles(branchName: string)}
	{@const active = activeSelectionId?.type === 'branch' && focusedStackId === stack.id}
	{@const changesResult = stackService.branchChanges({
		projectId,
		stackId: stack.id,
		branchName
	})}
	<ReduxResult {projectId} stackId={stack.id} result={changesResult.current}>
		{#snippet children(changes, { projectId, stackId })}
			<ChangedFiles
				title="Combined Changes"
				{projectId}
				{stackId}
				draggableFiles
				autoselect
				selectionId={{ type: 'branch', stackId: stack.id, branchName }}
				noshrink={!!previewKey}
				ontoggle={() => {
					changedFilesCollapsed = !changedFilesCollapsed;
				}}
				changes={changes.changes}
				{active}
				resizer={{
					persistId: `resizer-panel2-changed-files-${stack.id}`,
					defaultValue: undefined,
					maxHeight: maxChangedFilesHeight,
					minHeight: minChangedFilesHeight,
					passive: !previewKey,
					order: 1,
					unsetMaxHeight: previewKey ? unsetMaxHeight : undefined
				}}
			></ChangedFiles>
		{/snippet}
	</ReduxResult>
{/snippet}

<AsyncRender>
	<div
		bind:clientWidth
		bind:clientHeight
		class="stack-view-wrapper"
		role="presentation"
		class:dimmed
		tabindex="-1"
		data-id={stack.id}
		data-testid={TestId.Stack}
		data-testid-stackid={stack.id}
		data-testid-stack={stack.heads.at(0)?.name}
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
			parentId: DefinedFocusable.ViewportMiddle
		}}
	>
		<div
			class="stack-view"
			style:width={$persistedStackWidth + 'rem'}
			bind:this={stackViewEl}
			{@attach scrollingAttachment(intelligentScrollingService, stack.id, 'stack')}
		>
			{#if !isCommitting}
				<div class="drag-handle" data-remove-from-panning data-drag-handle draggable="true">
					<Icon name="draggable-narrow" rotate={90} />
				</div>
			{/if}
			<Resizer
				persistId="resizer-panel1-${stack.id}"
				viewport={stackViewEl!}
				zIndex="var(--z-lifted)"
				direction="right"
				minWidth={RESIZER_CONFIG.panel1.minWidth}
				maxWidth={RESIZER_CONFIG.panel1.maxWidth}
				defaultValue={RESIZER_CONFIG.panel1.defaultValue}
				syncName="panel1"
				imitateBorder
			/>
			<ReduxResult
				{projectId}
				result={combineResults(branchesResult.current, defaultBranchResult.current)}
			>
				{#snippet children([branches, defaultBranch])}
					<ConfigurableScrollableContainer>
						<!-- If we are currently committing, we should keep this open so users can actually stop committing again :wink: -->
						<div
							class="assignments-wrap"
							class:assignments__empty={changes.current.length === 0 && !isCommitting}
						>
							<div
								class="worktree-wrap"
								class:remove-border-bottom={(isCommitting && changes.current.length === 0) ||
									!startCommitVisible.current}
								class:dropzone-activated={dropzoneActivated && changes.current.length === 0}
							>
								<Changes
									title="Assigned"
									{projectId}
									stackId={stack.id}
									mode="assigned"
									active={focusedStackId === stack.id}
									dropzoneVisible={changes.current.length === 0 && !isCommitting}
									onDropzoneActivated={(activated) => {
										dropzoneActivated = activated;
									}}
									onselect={() => {
										// Clear one selection when you modify the other.
										stackState?.selection.set(undefined);
										intelligentScrollingService.show(projectId, stack.id, 'diff');
									}}
								>
									{#snippet emptyPlaceholder()}
										{#if !isCommitting}
											<div class="assigned-changes-empty">
												<p class="text-12 text-body assigned-changes-empty__text">
													Drop files to assign or commit directly
												</p>
											</div>
										{/if}
									{/snippet}
								</Changes>
							</div>

							{#if startCommitVisible.current || isCommitting}
								{#if !isCommitting}
									<div class="start-commit">
										<Button
											testId={TestId.StartCommitButton}
											kind={changes.current.length > 0 ? 'solid' : 'outline'}
											style={changes.current.length > 0 ? 'pop' : 'neutral'}
											type="button"
											wide
											disabled={defaultBranch === null || !!projectState.exclusiveAction.current}
											onclick={() => {
												if (defaultBranch) startCommit(defaultBranch);
											}}
										>
											Start a commit…
										</Button>
									</div>
								{:else if isCommitting}
									<NewCommitView {projectId} stackId={stack.id} />
								{/if}
							{/if}
						</div>

						<BranchList
							{projectId}
							{branches}
							stackId={stack.id}
							{focusedStackId}
							onselect={() => {
								// Clear one selection when you modify the other.
								idSelection.clear({ type: 'worktree', stackId: stack.id });
								intelligentScrollingService.show(projectId, stack.id, 'details');
							}}
						/>
					</ConfigurableScrollableContainer>
				{/snippet}
			</ReduxResult>
		</div>

		{#if commitId || branchName || assignedKey || selectedFile}
			<div
				class="combined-view"
				bind:this={compactDiv}
				bind:clientHeight={verticalHeight}
				data-remove-from-draggable
				data-details={stack.id}
			>
				{#if branchName && commitId}
					{@render commitView(branchName, commitId)}
					{@render commitChangedFiles(commitId)}
				{:else if branchName}
					{@render branchView(branchName)}
					{@render branchChangedFiles(branchName)}
				{/if}

				{#if assignedKey || selectedFile}
					<ReduxResult {projectId} result={previewChangeResult?.current}>
						{#snippet children(previewChange)}
							{@const diffResult = diffService.getDiff(projectId, previewChange)}
							{@const diffData = diffResult.current.data}

							{#if assignedKey?.type === 'worktree' && assignedKey.stackId}
								<ConfigurableScrollableContainer zIndex="var(--z-lifted)">
									{@render assignedChangePreview(assignedKey.stackId)}
								</ConfigurableScrollableContainer>
							{:else if selectedFile}
								<Drawer bottomBorder>
									{#snippet header()}
										<FileViewHeader
											noPaddings
											transparent
											filePath={previewChange.path}
											fileStatus={computeChangeStatus(previewChange)}
											linesAdded={diffData?.type === 'Patch'
												? diffData.subject.linesAdded
												: undefined}
											linesRemoved={diffData?.type === 'Patch'
												? diffData.subject.linesRemoved
												: undefined}
										/>
									{/snippet}
									{@render otherChangePreview(selectedFile)}
								</Drawer>
							{/if}
						{/snippet}
					</ReduxResult>
				{/if}

				<!-- The id of this resizer is intentionally the same as in default view. -->
				<Resizer
					viewport={compactDiv}
					persistId="resizer-panel2-${stack.id}"
					direction="right"
					minWidth={RESIZER_CONFIG.panel2.minWidth}
					maxWidth={RESIZER_CONFIG.panel2.maxWidth}
					defaultValue={RESIZER_CONFIG.panel2.defaultValue}
					syncName="panel2"
					imitateBorder
				/>
			</div>
		{/if}
	</div>
</AsyncRender>

<style lang="postcss">
	.stack-view-wrapper {
		display: flex;
		position: relative;
		flex-shrink: 0;
		height: 100%;
		overflow: hidden;
		transition: opacity 0.15s;

		&.dimmed {
			opacity: 0.5;
		}
	}

	.stack-view {
		display: flex;
		position: relative;
		flex-shrink: 0;
		flex-direction: column;
	}

	.dimmed .stack-view {
		pointer-events: none;
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

	.assignments-wrap {
		display: flex;
		flex-shrink: 0;
		flex-direction: column;
		margin: 12px;
		margin-bottom: 0;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.worktree-wrap {
		display: flex;
		flex-direction: column;
		border-bottom: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml) var(--radius-ml) 0 0;

		&.remove-border-bottom {
			border-bottom: none;
		}

		&.dropzone-activated {
			& .assigned-changes-empty {
				padding: 20px 8px 20px;
				background-color: var(--clr-bg-1);
				will-change: padding;
			}

			& .assigned-changes-empty__text {
				color: var(--clr-theme-pop-on-soft);
			}
		}
	}

	.combined-view {
		display: flex;
		position: relative;
		flex-shrink: 0;
		flex-direction: column;
		height: 100%;
		white-space: wrap;
	}

	.start-commit {
		padding: 12px;
		background-color: var(--clr-bg-1);
	}

	/* EMPTY ASSIGN AREA */
	.assigned-changes-empty {
		display: flex;
		position: relative;
		padding: 10px 8px;
		overflow: hidden;
		gap: 12px;
		background-color: var(--clr-bg-2);
		transition: background-color var(--transition-fast);
	}

	.drag-handle {
		display: flex;
		z-index: var(--z-floating);
		position: absolute;
		justify-content: flex-end;
		width: 100%;
		padding: 0 1px;
		color: var(--clr-text-2);
		cursor: grab;
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-text-1);
		}
	}
</style>
