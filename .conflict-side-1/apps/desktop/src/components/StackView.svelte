<script lang="ts">
	import BranchList from '$components/BranchList.svelte';
	import BranchView from '$components/BranchView.svelte';
	import ChangedFiles from '$components/ChangedFiles.svelte';
	import CommitView from '$components/CommitView.svelte';
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import Drawer from '$components/Drawer.svelte';
	import NewCommitView from '$components/NewCommitView.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import SelectionView from '$components/SelectionView.svelte';
	import WorktreeChanges from '$components/WorktreeChanges.svelte';
	import { stagingBehaviorFeature } from '$lib/config/uiFeatureFlags';
	import { isParsedError } from '$lib/error/parser';
	import { DIFF_SERVICE } from '$lib/hunks/diffService.svelte';
	import { FILE_SELECTION_MANAGER } from '$lib/selection/fileSelectionManager.svelte';
	import {
		createBranchSelection,
		createCommitSelection,
		createWorktreeSelection,
		readKey,
		type SelectionId
	} from '$lib/selection/key';
	import { UNCOMMITTED_SERVICE } from '$lib/selection/uncommittedService.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { mapResult } from '$lib/state/helpers';
	import { UI_STATE } from '$lib/state/uiState.svelte';

	import { createBranchRef } from '$lib/utils/branch';
	import { computeChangeStatus } from '$lib/utils/fileStatus';
	import { inject } from '@gitbutler/core/context';
	import { persistWithExpiration } from '@gitbutler/shared/persisted';

	import { Button, FileViewHeader, Icon, TestId } from '@gitbutler/ui';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import { intersectionObserver } from '@gitbutler/ui/utils/intersectionObserver';
	import { fly } from 'svelte/transition';
	import type { Commit } from '$lib/branches/v3';

	type Props = {
		projectId: string;
		stackId?: string;
		laneId: string;
		topBranch?: string;
		onVisible: (visible: boolean) => void;
		clientWidth?: number;
		clientHeight?: number;
	};

	let {
		projectId,
		stackId,
		laneId,
		topBranch,
		clientHeight = $bindable(),
		clientWidth = $bindable(),
		onVisible
	}: Props = $props();

	let lanesSrollableEl = $state<HTMLDivElement>();

	const stackService = inject(STACK_SERVICE);
	const diffService = inject(DIFF_SERVICE);
	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const uiState = inject(UI_STATE);
	const idSelection = inject(FILE_SELECTION_MANAGER);

	// Component is read-only when stackId is undefined
	const isReadOnly = $derived(!stackId);

	const projectState = $derived(uiState.project(projectId));

	const action = $derived(projectState.exclusiveAction.current);
	const isCommitting = $derived(action?.type === 'commit' && action.stackId === stackId);

	// If the user is making a commit to a different lane we dim this one.
	const dimmed = $derived(action?.type === 'commit' && action?.stackId !== stackId);

	// Get the default width from global state, but don't make it reactive during initialization
	const defaultStackWidth = uiState.global.stackWidth.current;
	const persistedStackWidth = persistWithExpiration(
		defaultStackWidth,
		`ui-stack-width-${stackId}`,
		1440
	);

	const branchesResult = $derived(stackService.branches(projectId, stackId));

	let dropzoneActivated = $state(false);

	const laneState = $derived(uiState.lane(laneId));
	const selection = $derived(laneState.selection);
	const assignedSelection = $derived(
		idSelection.getById(createWorktreeSelection({ stackId: stackId }))
	);
	const lastAddedAssigned = $derived(assignedSelection.lastAdded);
	const assignedKey = $derived(
		$lastAddedAssigned?.key ? readKey($lastAddedAssigned.key) : undefined
	);
	const assignedStackId = $derived(
		assignedKey?.type === 'worktree' ? assignedKey.stackId : undefined
	);

	const commitId = $derived(selection.current?.commitId);
	const branchName = $derived(selection.current?.branchName);
	const upstream = $derived(selection.current?.upstream);

	const activeSelectionId: SelectionId | undefined = $derived.by(() => {
		if (commitId) {
			return createCommitSelection({ commitId, stackId: stackId });
		} else if (branchName) {
			return createBranchSelection({ stackId: stackId, branchName, remote: undefined });
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

	const changes = $derived(uncommittedService.changesByStackId(stackId || null));

	let stackViewEl = $state<HTMLDivElement>();
	let compactDiv = $state<HTMLDivElement>();

	let changedFilesCollapsed = $state<boolean>();
	let active = $state(false);

	const defaultBranchResult = $derived(stackService.defaultBranch(projectId, stackId));
	const defaultBranch = $derived(defaultBranchResult?.current.data);

	// Resizer configuration for stack panels and details view
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

	function checkSelectedFilesForCommit() {
		const stackAssignments = stackId ? uncommittedService.getAssignmentsByStackId(stackId) : [];
		if (stackId && stackAssignments.length > 0) {
			// If there are assignments for this stack, we check those.
			const selectionId = createWorktreeSelection({ stackId });
			const selectedPaths = idSelection.values(selectionId).map((entry) => entry.path);

			// If there are selected paths, we check those.
			if (selectedPaths.length > 0) {
				uncommittedService.checkFiles(stackId, selectedPaths);
			} else {
				uncommittedService.checkAll(stackId);
			}
			// Uncheck the unassigned files.
			uncommittedService.uncheckAll(null);
			return;
		}

		const selectionId = createWorktreeSelection({});
		const selectedPaths = idSelection.values(selectionId).map((entry) => entry.path);

		// If there are selected paths in the unassigned selection, we check those.
		if (selectedPaths.length > 0) {
			uncommittedService.checkFiles(null, selectedPaths);
		} else {
			uncommittedService.checkAll(null);
		}
	}

	function uncheckAll() {
		if (stackId) {
			uncommittedService.uncheckAll(stackId);
		}
		uncommittedService.uncheckAll(null);
	}

	function checkAllFiles() {
		const stackAssignments = stackId ? uncommittedService.getAssignmentsByStackId(stackId) : [];
		if (stackId && stackAssignments.length > 0) {
			// If there are assignments for this stack, we check those.
			uncommittedService.checkAll(stackId);
			// Uncheck the unassigned files.
			uncommittedService.uncheckAll(null);
			return;
		}

		uncommittedService.checkAll(null);
	}

	function checkFilesForCommit(): true {
		switch ($stagingBehaviorFeature) {
			case 'all':
				checkAllFiles();
				return true;
			case 'selection':
				// We only check the selected files.
				checkSelectedFilesForCommit();
				return true;
			case 'none':
				uncheckAll();
				return true;
		}
	}

	function startCommit(branchName: string) {
		projectState.exclusiveAction.set({
			type: 'commit',
			branchName,
			stackId: stackId
		});

		checkFilesForCommit();
	}

	export function onclose() {
		selection.set(undefined);
	}

	const startCommitVisible = $derived(uncommittedService.startCommitVisible(stackId));

	function onerror(err: unknown) {
		// Clear selection if branch not found.
		if (isParsedError(err) && err.code === 'errors.branch.notfound') {
			selection?.set(undefined);
			console.warn('Workspace selection cleared');
		}
	}

	function getAncestorMostConflicted(commits: Commit[]): Commit | undefined {
		if (!commits.length) return undefined;
		for (let i = commits.length - 1; i >= 0; i--) {
			const commit = commits[i]!;
			if (commit.hasConflicts) {
				return commit;
			}
		}
		return undefined;
	}

	let isDetailsViewOpen = $derived(!!(branchName || commitId || assignedKey || selectedFile));
	const DETAILS_RIGHT_PADDING_REM = 1.125;

	// Function to update CSS custom property for details view width
	function updateDetailsViewWidth(width: number) {
		if (stackViewEl) {
			stackViewEl.style.setProperty('--details-view-width', `${width}rem`);
		}
	}

	// Set initial CSS custom properties when details view opens/closes
	$effect(() => {
		if (stackViewEl) {
			if (isDetailsViewOpen) {
				// Set default width if not already set or is zero
				const currentWidth = stackViewEl.style.getPropertyValue('--details-view-width');
				if (!currentWidth || currentWidth === '0rem') {
					stackViewEl.style.setProperty(
						'--details-view-width',
						`${RESIZER_CONFIG.panel2.defaultValue}rem`
					);
				}
			} else {
				stackViewEl.style.setProperty('--details-view-width', '0rem');
			}
		}
	});

	let selectionPreviewScrollContainer: HTMLDivElement | undefined = $state();
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
		testId={TestId.WorktreeSelectionView}
		{projectId}
		scrollContainer={selectionPreviewScrollContainer}
		selectionId={createWorktreeSelection({ stackId })}
		onclose={() => {
			idSelection.clear(createWorktreeSelection({ stackId: stackId }));
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
		onclose={() => {}}
		draggableFiles={selectionId.type === 'commit'}
	/>
{/snippet}

{#snippet branchView(branchName: string)}
	<BranchView {stackId} {laneId} {projectId} {branchName} {onerror} {onclose} />
{/snippet}

{#snippet commitView(branchName: string, commitId: string)}
	<CommitView
		{projectId}
		{stackId}
		{laneId}
		commitKey={{
			stackId: stackId,
			branchName,
			commitId,
			upstream: !!upstream
		}}
		draggableFiles
		{onerror}
		{onclose}
	/>
{/snippet}

{#snippet commitChangedFiles(commitId: string)}
	{@const changesResult = stackService.commitChanges(projectId, commitId)}
	<ReduxResult
		{projectId}
		{stackId}
		result={mapResult(changesResult.current, (changes) => ({ changes, commitId }))}
	>
		{#snippet children({ changes, commitId }, { projectId, stackId })}
			{@const commitsResult = branchName
				? stackService.commits(projectId, stackId, branchName)
				: undefined}
			{@const commits = commitsResult?.current.data || []}
			{@const ancestorMostConflictedCommitId = getAncestorMostConflicted(commits)?.id}

			<ChangedFiles
				title="Changed files"
				{projectId}
				{stackId}
				draggableFiles
				selectionId={createCommitSelection({ commitId, stackId: stackId })}
				noshrink={!!previewKey}
				ontoggle={(collapsed) => {
					changedFilesCollapsed = collapsed;
				}}
				changes={changes.changes.filter(
					(change) => !(change.path in (changes.conflictEntries?.entries ?? {}))
				)}
				stats={changes.stats}
				conflictEntries={changes.conflictEntries}
				{ancestorMostConflictedCommitId}
				resizer={{
					persistId: `changed-files-${stackId}`,
					direction: 'down',
					minHeight: 8,
					maxHeight: 32,
					defaultValue: 16
				}}
				autoselect
			/>
		{/snippet}
	</ReduxResult>
{/snippet}

{#snippet branchChangedFiles(branchName: string)}
	{@const changesResult = stackService.branchChanges({
		projectId,
		stackId: stackId,
		branch: createBranchRef(branchName, undefined)
	})}
	<ReduxResult {projectId} {stackId} result={changesResult.current}>
		{#snippet children(changes, { projectId, stackId })}
			<ChangedFiles
				title="Combined Changes"
				{projectId}
				{stackId}
				draggableFiles
				autoselect
				selectionId={createBranchSelection({ stackId: stackId, branchName, remote: undefined })}
				noshrink={!!previewKey}
				ontoggle={() => {
					changedFilesCollapsed = !changedFilesCollapsed;
				}}
				changes={changes.changes}
				stats={changes.stats}
				resizer={{
					persistId: `changed-files-${stackId}`,
					direction: 'down',
					minHeight: 8,
					maxHeight: 32,
					defaultValue: 16
				}}
			/>
		{/snippet}
	</ReduxResult>
{/snippet}

<div
	bind:clientWidth
	bind:clientHeight
	class="stack-view-wrapper"
	role="presentation"
	class:dimmed
	tabindex="-1"
	data-id={stackId}
	data-testid={TestId.Stack}
	data-testid-stackid={stackId}
	data-testid-stack={topBranch}
	use:intersectionObserver={{
		callback: (entry) => {
			onVisible(!!entry?.isIntersecting);
		},
		options: {
			threshold: 0.5,
			root: lanesSrollableEl
		}
	}}
	use:focusable
>
	{#if !isCommitting}
		<div class="drag-handle" data-remove-from-panning data-drag-handle draggable="true">
			<Icon name="draggable-narrow" rotate={90} noEvents />
		</div>
	{/if}

	<ConfigurableScrollableContainer childrenWrapHeight="100%">
		<div
			class="stack-view"
			class:details-open={isDetailsViewOpen}
			style:width="{$persistedStackWidth}rem"
			use:focusable={{ vertical: true, onActive: (value) => (active = value) }}
			bind:this={stackViewEl}
		>
			<ReduxResult {projectId} result={branchesResult.current}>
				{#snippet children(branches)}
					<div class="stack-v">
						<!-- If we are currently committing, we should keep this open so users can actually stop committing again :wink: -->
						<div
							class="assignments-wrap"
							class:assignments__empty={changes.current.length === 0 && !isCommitting}
							use:focusable={{ vertical: true }}
						>
							<div
								class="worktree-wrap"
								class:remove-border-bottom={(isCommitting && changes.current.length === 0) ||
									!startCommitVisible.current}
								class:dropzone-activated={dropzoneActivated && changes.current.length === 0}
							>
								<WorktreeChanges
									title="Assigned"
									{projectId}
									{stackId}
									mode="assigned"
									dropzoneVisible={changes.current.length === 0 && !isCommitting}
									onDropzoneActivated={(activated) => {
										dropzoneActivated = activated;
									}}
									onselect={() => {
										// Clear one selection when you modify the other.
										laneState?.selection.set(undefined);
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
								</WorktreeChanges>
							</div>

							{#if startCommitVisible.current || isCommitting}
								{#if !isCommitting}
									<div class="start-commit" use:focusable>
										<Button
											testId={TestId.StartCommitButton}
											kind={changes.current.length > 0 ? 'solid' : 'outline'}
											style={changes.current.length > 0 ? 'pop' : 'neutral'}
											type="button"
											wide
											disabled={isReadOnly ||
												defaultBranch === null ||
												!!projectState.exclusiveAction.current}
											tooltip={isReadOnly ? 'Read-only mode' : undefined}
											onclick={() => {
												if (defaultBranch) startCommit(defaultBranch);
											}}
										>
											Start a commit…
										</Button>
									</div>
								{:else if isCommitting}
									<NewCommitView {projectId} {stackId} />
								{/if}
							{/if}
						</div>

						<BranchList
							{projectId}
							{branches}
							{laneId}
							{stackId}
							{active}
							onselect={() => {
								// Clear one selection when you modify the other.
								idSelection.clear({ type: 'worktree', stackId: stackId });
							}}
						/>
					</div>

					<!-- RESIZE PANEL 1 -->
					<Resizer
						persistId="resizer-panel1-${stackId}"
						viewport={stackViewEl!}
						zIndex="var(--z-lifted)"
						direction="right"
						minWidth={RESIZER_CONFIG.panel1.minWidth}
						maxWidth={RESIZER_CONFIG.panel1.maxWidth}
						defaultValue={RESIZER_CONFIG.panel1.defaultValue}
						syncName="panel1"
						onWidth={(newWidth) => {
							// Update the persisted stack width when panel1 resizer changes
							persistedStackWidth.set(newWidth);
						}}
					/>
				{/snippet}
			</ReduxResult>
		</div>
	</ConfigurableScrollableContainer>

	<!-- PREVIEW -->
	{#if isDetailsViewOpen}
		<div
			in:fly={{ y: 20, duration: 200 }}
			class="details-view"
			bind:this={compactDiv}
			data-details={stackId}
			style:right="{DETAILS_RIGHT_PADDING_REM}rem"
			use:focusable={{ vertical: true }}
		>
			<div class="details-view__inner">
				<!-- TOP SECTION: Branch/Commit Details (no resizer) -->
				{#if branchName && commitId}
					{@render commitView(branchName, commitId)}
				{:else if branchName}
					{@render branchView(branchName)}
				{/if}

				<!-- MIDDLE SECTION: Changed Files (with resizer) -->
				{#if branchName && commitId}
					{@render commitChangedFiles(commitId)}
				{:else if branchName}
					{@render branchChangedFiles(branchName)}
				{/if}

				<!-- BOTTOM SECTION: File Preview (no resizer) -->
				{#if assignedStackId || selectedFile}
					<ReduxResult {projectId} result={previewChangeResult?.current}>
						{#snippet children(previewChange)}
							{@const diffResult = diffService.getDiff(projectId, previewChange)}
							{@const diffData = diffResult.current.data}

							<div class="file-preview-section">
								{#if assignedStackId}
									<ConfigurableScrollableContainer
										zIndex="var(--z-lifted)"
										bind:viewport={selectionPreviewScrollContainer}
									>
										{@render assignedChangePreview(assignedStackId)}
									</ConfigurableScrollableContainer>
								{:else if selectedFile}
									<Drawer>
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
							</div>
						{/snippet}
					</ReduxResult>
				{/if}
			</div>
		</div>

		<!-- DETAILS VIEW WIDTH RESIZER - Only show when details view is open -->
		<Resizer
			viewport={compactDiv!}
			persistId="resizer-panel2-${stackId}"
			direction="right"
			minWidth={RESIZER_CONFIG.panel2.minWidth}
			maxWidth={RESIZER_CONFIG.panel2.maxWidth}
			defaultValue={RESIZER_CONFIG.panel2.defaultValue}
			syncName="panel2"
			onWidth={updateDetailsViewWidth}
		/>
	{/if}
</div>

<style lang="postcss">
	.stack-view-wrapper {
		display: flex;
		position: relative;
		flex-shrink: 0;
		height: 100%;
		overflow: hidden;
		border-right: 1px solid var(--clr-border-2);
		transition: opacity 0.15s;

		&.dimmed {
			opacity: 0.5;
		}

		&:focus {
			outline: none;
		}
	}

	.stack-view {
		display: flex;
		position: relative;
		flex-shrink: 0;
		flex-direction: column;
		padding: 0 12px;

		/* Use CSS custom properties for details view width to avoid ResizeObserver errors */
		--details-view-width: 0rem;
	}

	.stack-view.details-open {
		margin-right: calc(var(--details-view-width) + 1.125rem);
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
		margin-top: 12px;
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

	.details-view {
		display: flex;
		z-index: var(--z-ground);
		position: absolute;
		top: 12px;
		flex-shrink: 0;
		flex-direction: column;
		max-height: calc(100% - 24px);
		margin-right: 2px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
		box-shadow: 0 10px 30px 0 color(srgb 0 0 0 / 0.16);
	}

	/* Needed for `focusCursor.svelte` to work correctly on `Drawer` components . */
	.details-view__inner {
		display: flex;
		position: relative;
		flex-direction: column;
		overflow: hidden;
	}

	:global(.dark) .details-view {
		box-shadow: 0 10px 50px 5px color(srgb 0 0 0 / 0.5);
	}

	.file-preview-section {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-height: 0; /* Allow shrinking */
		overflow: hidden;
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
