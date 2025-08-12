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
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UI_STATE } from '$lib/state/uiState.svelte';

	import { computeChangeStatus } from '$lib/utils/fileStatus';
	import { inject } from '@gitbutler/shared/context';
	import { persistWithExpiration } from '@gitbutler/shared/persisted';

	import { Button, FileViewHeader, Icon, TestId } from '@gitbutler/ui';
	import { intersectionObserver } from '@gitbutler/ui/utils/intersectionObserver';
	import { fly } from 'svelte/transition';
	import type { Commit } from '$lib/branches/v3';
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

	// If the user is making a commit to a different lane we dim this one.
	const dimmed = $derived(action?.type === 'commit' && action?.stackId !== stack.id);

	// Get the default width from global state, but don't make it reactive during initialization
	const defaultStackWidth = uiState.global.stackWidth.current;
	const persistedStackWidth = persistWithExpiration(
		defaultStackWidth,
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

	let stackViewEl = $state<HTMLDivElement>();
	let compactDiv = $state<HTMLDivElement>();

	let changedFilesCollapsed = $state<boolean>();

	const defaultBranchResult = $derived(stackService.defaultBranch(projectId, stack.id));

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
		{onerror}
		{onclose}
	/>
{/snippet}

{#snippet commitChangedFiles(commitId: string)}
	{@const active = activeSelectionId?.type === 'commit' && focusedStackId === stack.id}
	{@const changesResult = stackService.commitChanges(projectId, commitId)}
	<ReduxResult {projectId} stackId={stack.id} result={changesResult.current}>
		{#snippet children(changes, { projectId, stackId })}
			{@const commitsResult = branchName
				? stackService.commits(projectId, stackId, branchName)
				: undefined}
			{@const commits = commitsResult?.current.data || []}
			{@const ancestorMostConflictedCommitId = getAncestorMostConflicted(commits)?.id}

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
				stats={changes.stats}
				conflictEntries={changes.conflictEntries}
				{ancestorMostConflictedCommitId}
				{active}
				resizer={{
					persistId: `changed-files-${stack.id}`,
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
				stats={changes.stats}
				{active}
				resizer={{
					persistId: `changed-files-${stack.id}`,
					direction: 'down',
					minHeight: 8,
					maxHeight: 32,
					defaultValue: 16
				}}
			></ChangedFiles>
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
	{#if !isCommitting}
		<div class="drag-handle" data-remove-from-panning data-drag-handle draggable="true">
			<Icon name="draggable-narrow" rotate={90} noEvents />
		</div>
	{/if}

	<ConfigurableScrollableContainer childrenWrapHeight="100%">
		<div
			class="stack-view"
			class:details-open={isDetailsViewOpen}
			{@attach scrollingAttachment(intelligentScrollingService, stack.id, 'stack')}
			style:width="{$persistedStackWidth}rem"
			bind:this={stackViewEl}
		>
			<ReduxResult
				{projectId}
				result={combineResults(branchesResult.current, defaultBranchResult.current)}
			>
				{#snippet children([branches, defaultBranch])}
					<div class="stack-v">
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
								</WorktreeChanges>
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
					</div>

					<!-- RESIZE PANEL 1 -->
					<Resizer
						persistId="resizer-panel1-${stack.id}"
						viewport={stackViewEl!}
						zIndex="var(--z-lifted)"
						direction="right"
						minWidth={RESIZER_CONFIG.panel1.minWidth}
						maxWidth={RESIZER_CONFIG.panel1.maxWidth}
						defaultValue={RESIZER_CONFIG.panel1.defaultValue}
						syncName="panel1"
					/>
				{/snippet}
			</ReduxResult>
		</div>
	</ConfigurableScrollableContainer>

	<!-- STACK WIDTH RESIZER -->
	<Resizer
		persistId="ui-stack-width-${stack.id}"
		viewport={stackViewEl!}
		zIndex="var(--z-lifted)"
		direction="right"
		minWidth={18}
		maxWidth={64}
		defaultValue={defaultStackWidth}
		onWidth={(newWidth) => {
			// Update the persisted stack width when resizer changes
			persistedStackWidth.set(newWidth);
		}}
	/>

	<!-- PREVIEW -->
	{#if isDetailsViewOpen}
		<div
			in:fly={{ y: 20, duration: 200 }}
			class="details-view"
			bind:this={compactDiv}
			data-details={stack.id}
			style:right="{DETAILS_RIGHT_PADDING_REM}rem"
		>
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
			{#if assignedKey || selectedFile}
				<ReduxResult {projectId} result={previewChangeResult?.current}>
					{#snippet children(previewChange)}
						{@const diffResult = diffService.getDiff(projectId, previewChange)}
						{@const diffData = diffResult.current.data}

						<div class="file-preview-section">
							{#if assignedKey?.type === 'worktree' && assignedKey.stackId}
								<ConfigurableScrollableContainer zIndex="var(--z-lifted)">
									{@render assignedChangePreview(assignedKey.stackId)}
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

		<!-- DETAILS VIEW WIDTH RESIZER -->
		<Resizer
			viewport={compactDiv!}
			persistId="resizer-panel2-${stack.id}"
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
	}

	.stack-view {
		display: flex;
		position: relative;
		flex-shrink: 0;
		flex-direction: column;
		height: 100%;
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
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
		box-shadow: 0 10px 30px 0 color(srgb 0 0 0 / 0.16);
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
