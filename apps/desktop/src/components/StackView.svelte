<script lang="ts">
	import BranchList from '$components/BranchList.svelte';
	import BranchView from '$components/BranchView.svelte';
	import CardOverlay from '$components/CardOverlay.svelte';
	import ChangedFiles from '$components/ChangedFiles.svelte';
	import CommitView from '$components/CommitView.svelte';
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import Drawer from '$components/Drawer.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import FullviewLoading from '$components/FullviewLoading.svelte';
	import NewCommitView from '$components/NewCommitView.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import SelectionView from '$components/SelectionView.svelte';
	import StackDragHandle from '$components/StackDragHandle.svelte';
	import WorktreeChanges from '$components/WorktreeChanges.svelte';
	import CodegenMcpConfigModal from '$components/codegen/CodegenMcpConfigModal.svelte';
	import CodegenMessages from '$components/codegen/CodegenMessages.svelte';
	import { isLocalAndRemoteCommit, isUpstreamCommit } from '$components/lib';
	import { ATTACHMENT_SERVICE } from '$lib/codegen/attachmentService.svelte';
	import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
	import { MessageSender } from '$lib/codegen/messageQueue.svelte';
	import {
		AmendCommitWithChangeDzHandler,
		AmendCommitWithHunkDzHandler,
		createCommitDropHandlers,
		type DzCommitData
	} from '$lib/commits/dropHandler';
	import { projectRunCommitHooks } from '$lib/config/config';
	import { stagingBehaviorFeature } from '$lib/config/uiFeatureFlags';
	import { isParsedError } from '$lib/error/parser';
	import { HOOKS_SERVICE } from '$lib/hooks/hooksService';
	import { DIFF_SERVICE } from '$lib/hunks/diffService.svelte';
	import { RULES_SERVICE } from '$lib/rules/rulesService.svelte';
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
	import { combineResults } from '$lib/state/helpers';
	import { UI_STATE } from '$lib/state/uiState.svelte';

	import { createBranchRef } from '$lib/utils/branch';
	import { computeChangeStatus } from '$lib/utils/fileStatus';
	import { inject } from '@gitbutler/core/context';
	import { persistWithExpiration } from '@gitbutler/shared/persisted';

	import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
	import { Button, FileViewHeader, TestId } from '@gitbutler/ui';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import { intersectionObserver } from '@gitbutler/ui/utils/intersectionObserver';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import { fly } from 'svelte/transition';
	import type { Commit } from '$lib/branches/v3';

	type Props = {
		projectId: string;
		stackId: string | undefined;
		laneId: string;
		topBranchName?: string;
		onVisible: (visible: boolean) => void;
		clientWidth?: number;
		clientHeight?: number;
	};

	let {
		projectId,
		stackId,
		laneId,
		topBranchName,
		clientHeight = $bindable(),
		clientWidth = $bindable(),
		onVisible
	}: Props = $props();

	const stableStackId = $derived(stackId);
	const stableProjectId = $derived(projectId);
	let lanesSrollableEl = $state<HTMLDivElement>();

	const stackService = inject(STACK_SERVICE);
	const diffService = inject(DIFF_SERVICE);
	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const uiState = inject(UI_STATE);
	const hooksService = inject(HOOKS_SERVICE);
	const idSelection = inject(FILE_SELECTION_MANAGER);

	// Component is read-only when stackId is undefined
	const isReadOnly = $derived(!stableStackId);

	const projectState = $derived(uiState.project(stableProjectId));

	const action = $derived(projectState.exclusiveAction.current);
	const isCommitting = $derived(action?.type === 'commit' && action.stackId === stableStackId);

	// If the user is making a commit to a different lane we dim this one.
	const dimmed = $derived(action?.type === 'commit' && action?.stackId !== stableStackId);

	// Resizer configuration for stack panels and details view
	const RESIZER_CONFIG: {
		panel1: { minWidth: number; maxWidth: number; defaultValue: number };
		panel2: { minWidth: number; maxWidth: number; defaultValue: number };
	} = {
		panel1: {
			minWidth: 20,
			maxWidth: 64,
			defaultValue: 23
		},
		panel2: {
			minWidth: 20,
			maxWidth: 64,
			defaultValue: 32
		}
	};

	const persistedStackWidth = $derived(
		persistWithExpiration(
			RESIZER_CONFIG.panel1.defaultValue,
			`ui-stack-width-${stableStackId}`,
			1440
		)
	);

	const branchesQuery = $derived(stackService.branches(stableProjectId, stableStackId));

	let active = $state(false);

	let dropzoneActivated = $state(false);

	const laneState = $derived(uiState.lane(laneId));
	const selection = $derived(laneState.selection);
	const assignedSelection = $derived(
		idSelection.getById(createWorktreeSelection({ stackId: stableStackId }))
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
	const previewOpen = $derived(selection.current?.previewOpen);

	// Get commit data for drop handlers when viewing a commit
	const commitQuery = $derived(
		commitId ? stackService.commitById(stableProjectId, stableStackId, commitId) : undefined
	);
	const runHooks = $derived(projectRunCommitHooks(stableProjectId));
	const isCommitView = $derived(!!(branchName && commitId));

	const activeSelectionId: SelectionId | undefined = $derived.by(() => {
		if (commitId) {
			return createCommitSelection({ commitId, stackId: stableStackId });
		} else if (branchName) {
			return createBranchSelection({ stackId: stableStackId, branchName, remote: undefined });
		}
	});

	const activeLastAdded = $derived.by(() => {
		if (activeSelectionId) {
			return idSelection.getById(activeSelectionId).lastAdded;
		}
	});

	const selectedFile = $derived($activeLastAdded?.key ? readKey($activeLastAdded.key) : undefined);

	const previewKey = $derived(assignedKey || selectedFile);
	const previewChangeQuery = $derived(
		previewKey ? idSelection.changeByKey(stableProjectId, previewKey) : undefined
	);

	const changes = $derived(uncommittedService.changesByStackId(stableStackId || null));

	let stackViewEl = $state<HTMLDivElement>();
	let compactDiv = $state<HTMLDivElement>();

	const defaultBranchQuery = $derived(stackService.defaultBranch(stableProjectId, stableStackId));
	const defaultBranch = $derived(defaultBranchQuery?.response);

	function checkSelectedFilesForCommit() {
		const stackAssignments = stableStackId
			? uncommittedService.getAssignmentsByStackId(stableStackId)
			: [];
		if (stableStackId && stackAssignments.length > 0) {
			// If there are assignments for this stack, we check those.
			const selectionId = createWorktreeSelection({ stackId: stableStackId });
			const selectedPaths = idSelection.values(selectionId).map((entry) => entry.path);

			// If there are selected paths, we check those.
			if (selectedPaths.length > 0) {
				uncommittedService.checkFiles(stableStackId, selectedPaths);
			} else {
				uncommittedService.checkAll(stableStackId);
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
		if (stableStackId) {
			uncommittedService.uncheckAll(stableStackId);
		}
		uncommittedService.uncheckAll(null);
	}

	function checkAllFiles() {
		const stackAssignments = stableStackId
			? uncommittedService.getAssignmentsByStackId(stableStackId)
			: [];
		if (stableStackId && stackAssignments.length > 0) {
			// If there are assignments for this stack, we check those.
			uncommittedService.checkAll(stableStackId);
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
			stackId: stableStackId
		});

		checkFilesForCommit();
	}

	function onclosePreview() {
		// Clear file selections for the active branch or commit
		if (activeSelectionId) {
			idSelection.clear(activeSelectionId);
		}
		// Clear the lane selection (branch/commit)
		selection.set(undefined);
	}

	const startCommitVisible = $derived(uncommittedService.startCommitVisible(stableStackId));

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

	// Details view can be opened in two ways:
	// 1. User selects a branch/commit/file and opens preview
	// 2. Files are assigned to this stack (assignedKey exists)
	const hasActiveSelection = $derived(!!(branchName || commitId || selectedFile));
	const isPreviewOpenForSelection = $derived(hasActiveSelection && previewOpen);
	const hasAssignedFiles = $derived(!!assignedKey);

	let isDetailsViewOpen = $derived(isPreviewOpenForSelection || hasAssignedFiles);

	const DETAILS_RIGHT_PADDING_REM = 1.125;

	// Function to update CSS custom property for details view width
	function updateDetailsViewWidth(width: number) {
		if (stackViewEl) {
			stackViewEl.style.setProperty('--details-view-width', `${width}rem`);
		}
	}

	// Set initial CSS custom properties when details view opens/closes
	$effect(() => {
		const element = stackViewEl;
		if (element) {
			if (isDetailsViewOpen) {
				// Set default width if not already set or is zero
				const currentWidth = element.style.getPropertyValue('--details-view-width');
				if (!currentWidth || currentWidth === '0rem') {
					element.style.setProperty(
						'--details-view-width',
						`${RESIZER_CONFIG.panel2.defaultValue}rem`
					);
				}
			} else {
				element.style.setProperty('--details-view-width', '0rem');
			}
		}

		// Cleanup function to reset the property when the effect reruns or component unmounts
		return () => {
			if (element) {
				element.style.removeProperty('--details-view-width');
			}
		};
	});

	let selectionPreviewScrollContainer: HTMLDivElement | undefined = $state();

	// Codegen related services and state
	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);
	const rulesService = inject(RULES_SERVICE);
	const attachmentService = inject(ATTACHMENT_SERVICE);

	let mcpConfigModal = $state<CodegenMcpConfigModal>();

	const mcpConfigQuery = $derived(claudeCodeService.mcpConfig({ projectId }));
	const isStackActiveQuery = $derived(claudeCodeService.isStackActive(projectId, stackId));
	const isStackActive = $derived(isStackActiveQuery?.response || false);
	const events = $derived(claudeCodeService.messages({ projectId, stackId }));
	const sessionId = $derived(rulesService.aiSessionId(projectId, stackId));
	const hasRulesToClear = $derived(rulesService.hasRulesToClear(projectId, stackId));
	const permissionRequests = $derived(claudeCodeService.permissionRequests({ projectId }));
	const attachments = $derived(attachmentService.getByBranch(branchName));

	const selectedThinkingLevel = $derived(projectState.thinkingLevel.current);
	const selectedModel = $derived(projectState.selectedModel.current);
	const selectedPermissionMode = $derived(uiState.lane(laneId).permissionMode.current);

	const messageSender = $derived(
		stackId && branchName
			? new MessageSender({
					projectId: reactive(() => projectId),
					selectedBranch: reactive(() => ({ stackId, head: branchName })),
					thinkingLevel: reactive(() => selectedThinkingLevel),
					model: reactive(() => selectedModel),
					permissionMode: reactive(() => selectedPermissionMode)
				})
			: undefined
	);
	const initialPrompt = $derived(messageSender?.prompt);

	async function onAbort() {
		if (stackId) {
			await claudeCodeService.cancelSession({ projectId, stackId });
		}
	}

	async function sendMessage(prompt: string) {
		await messageSender?.sendMessage(prompt, attachments);
		attachmentService.clearByBranch(branchName);
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
		testId={TestId.WorktreeSelectionView}
		projectId={stableProjectId}
		scrollContainer={selectionPreviewScrollContainer}
		selectionId={createWorktreeSelection({ stackId })}
		onclose={() => {
			idSelection.clearPreview(createWorktreeSelection({ stackId: stackId }));
		}}
		draggableFiles
	/>
{/snippet}

{#snippet otherChangePreview(selectionId: SelectionId)}
	<SelectionView
		testId={TestId.StackSelectionView}
		projectId={stableProjectId}
		{selectionId}
		diffOnly={true}
		draggableFiles={selectionId.type === 'commit'}
	/>
{/snippet}

{#snippet branchView(branchName: string)}
	<BranchView
		stackId={stableStackId}
		{laneId}
		projectId={stableProjectId}
		{branchName}
		{onerror}
		onclose={onclosePreview}
	/>
{/snippet}

{#snippet commitView(branchName: string, commitId: string)}
	<CommitView
		projectId={stableProjectId}
		stackId={stableStackId}
		{laneId}
		commitKey={{
			stackId: stableStackId,
			branchName,
			commitId,
			upstream: !!upstream
		}}
		draggableFiles
		{onerror}
		onclose={onclosePreview}
	/>
{/snippet}

{#snippet commitChangedFiles(commitId: string)}
	{@const changesQuery = stackService.commitChanges(stableProjectId, commitId)}
	<ReduxResult projectId={stableProjectId} stackId={stableStackId} result={changesQuery.result}>
		{#snippet children(changesResult, { projectId, stackId })}
			{@const commitsQuery = branchName
				? stackService.commits(projectId, stackId, branchName)
				: undefined}
			{@const commits = commitsQuery?.response || []}
			{@const ancestorMostConflictedCommitId = getAncestorMostConflicted(commits)?.id}

			<ChangedFiles
				title="Changed files"
				{projectId}
				{stackId}
				draggableFiles
				selectionId={createCommitSelection({ commitId, stackId: stackId })}
				noshrink={!!previewKey}
				grow={!previewKey}
				persistId={`commit-${commitId}`}
				changes={changesResult.changes.filter(
					(change) => !(change.path in (changesResult.conflictEntries?.entries ?? {}))
				)}
				stats={changesResult.stats}
				conflictEntries={changesResult.conflictEntries}
				{ancestorMostConflictedCommitId}
				resizer={previewKey
					? {
							persistId: `changed-files-${stackId}`,
							direction: 'down',
							minHeight: 8,
							maxHeight: 32,
							defaultValue: 16
						}
					: undefined}
				autoselect
				allowUnselect={false}
			/>
		{/snippet}
	</ReduxResult>
{/snippet}

{#snippet branchChangedFiles(branchName: string)}
	{@const changesQuery = stackService.branchChanges({
		projectId: stableProjectId,
		stackId: stableStackId,
		branch: createBranchRef(branchName, undefined)
	})}
	<ReduxResult projectId={stableProjectId} stackId={stableStackId} result={changesQuery.result}>
		{#snippet children(changesResult, { projectId, stackId })}
			<ChangedFiles
				title="Combined Changes"
				{projectId}
				{stackId}
				draggableFiles
				autoselect
				selectionId={createBranchSelection({ stackId: stackId, branchName, remote: undefined })}
				noshrink={!!previewKey}
				grow={!previewKey}
				persistId={`branch-${branchName}`}
				changes={changesResult.changes}
				stats={changesResult.stats}
				resizer={previewKey
					? {
							persistId: `changed-files-${stackId}`,
							direction: 'down',
							minHeight: 8,
							maxHeight: 32,
							defaultValue: 16
						}
					: undefined}
				allowUnselect={false}
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
	data-id={stableStackId}
	data-testid={TestId.Stack}
	data-testid-stackid={stableStackId}
	data-testid-stack={topBranchName}
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
		onKeydown: (event) => {
			if (event.key === 'Escape' && isDetailsViewOpen) {
				onclosePreview();
				event.preventDefault();
				event.stopPropagation();
				return true;
			}
		}
	}}
>
	<ReduxResult
		projectId={stableProjectId}
		result={combineResults(branchesQuery.result, hasRulesToClear.result)}
	>
		{#snippet loading()}
			<div style:width="{$persistedStackWidth}rem" class="lane-skeleton">
				<FullviewLoading />
			</div>
		{/snippet}
		{#snippet children([branches, hasRulesToClear])}
			<ConfigurableScrollableContainer childrenWrapHeight="100%">
				<div
					class="stack-view"
					class:details-open={isDetailsViewOpen}
					style:width="{$persistedStackWidth}rem"
					use:focusable={{ vertical: true, onActive: (value) => (active = value) }}
					bind:this={stackViewEl}
				>
					<div class="stack-v" data-fade-on-reorder>
						<!-- If we are currently committing, we should keep this open so users can actually stop committing again :wink: -->
						<StackDragHandle stackId={stableStackId} {projectId} disabled={isCommitting} />

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
									projectId={stableProjectId}
									stackId={stableStackId}
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
									<div class="start-commit">
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
									<NewCommitView projectId={stableProjectId} stackId={stableStackId} />
								{/if}
							{/if}
						</div>

						<BranchList
							projectId={stableProjectId}
							{branches}
							{laneId}
							stackId={stableStackId}
							{active}
							onselect={() => {
								// Clear one selection when you modify the other.
								idSelection.clear({ type: 'worktree', stackId: stableStackId });
							}}
						/>
					</div>

					<!-- RESIZE PANEL 1 -->
					{#if stackViewEl}
						<Resizer
							persistId="ui-stack-width-${stableStackId}"
							viewport={stackViewEl}
							zIndex="var(--z-lifted)"
							direction="right"
							showBorder={!isDetailsViewOpen}
							minWidth={RESIZER_CONFIG.panel1.minWidth}
							maxWidth={RESIZER_CONFIG.panel1.maxWidth}
							defaultValue={$persistedStackWidth ?? RESIZER_CONFIG.panel1.defaultValue}
							syncName="panel1"
							onWidth={(newWidth) => {
								// Update the persisted stack width when panel1 resizer changes
								persistedStackWidth.set(newWidth);
							}}
						/>
					{/if}
				</div>
			</ConfigurableScrollableContainer>

			<!-- PREVIEW -->
			{#if isDetailsViewOpen}
				{@const selection = laneState.selection.current}
				<div
					in:fly={{ y: 20, duration: 200 }}
					class="details-view deep-shadow"
					bind:this={compactDiv}
					data-details={stableStackId}
					style:right="{DETAILS_RIGHT_PADDING_REM}rem"
					use:focusable={{ vertical: true }}
					data-testid={TestId.StackPreview}
				>
					{#if stableStackId && selection?.branchName && selection?.codegen}
						<CodegenMessages
							projectId={stableProjectId}
							stackId={stableStackId}
							{laneId}
							branchName={selection.branchName}
							onclose={onclosePreview}
							onMcpSettings={() => {
								mcpConfigModal?.open();
							}}
							{onAbort}
							{initialPrompt}
							events={events.response || []}
							permissionRequests={permissionRequests.response || []}
							onSubmit={sendMessage}
							onChange={(prompt) => messageSender?.setPrompt(prompt)}
							sessionId={sessionId.response}
							{isStackActive}
							{hasRulesToClear}
						/>
					{:else}
						{@const commit = commitQuery?.response}
						{@const dzCommit: DzCommitData | undefined = commit
							? {
									id: commit.id,
									isRemote: isUpstreamCommit(commit),
									isIntegrated:
										isLocalAndRemoteCommit(commit) && commit.state.type === 'Integrated',
									hasConflicts: isLocalAndRemoteCommit(commit) && commit.hasConflicts
								}
							: undefined}
						{@const { amendHandler, squashHandler, hunkHandler } =
							isCommitView && dzCommit
								? createCommitDropHandlers({
										projectId: stableProjectId,
										stackId: stableStackId,
										stackService,
										hooksService,
										uiState,
										commit: dzCommit,
										runHooks: $runHooks,
										okWithForce: true,
										onCommitIdChange: (newId) => {
											if (stableStackId && branchName && selection) {
												const previewOpen = selection.previewOpen ?? true;
												uiState
													.lane(stableStackId)
													.selection.set({ branchName, commitId: newId, previewOpen });
											}
										}
									})
								: { amendHandler: undefined, squashHandler: undefined, hunkHandler: undefined }}
						{#if isCommitView}
							<Dropzone
								handlers={[amendHandler, squashHandler, hunkHandler].filter(isDefined)}
								fillHeight
							>
								{#snippet overlay({ hovered, activated, handler })}
									{@const label =
										handler instanceof AmendCommitWithChangeDzHandler ||
										handler instanceof AmendCommitWithHunkDzHandler
											? 'Amend'
											: 'Squash'}
									<CardOverlay {hovered} {activated} {label} />
								{/snippet}
								<div class="details-view__inner">
									<!-- TOP SECTION: Branch/Commit Details (no resizer) -->
									{@render commitView(branchName!, commitId!)}

									<!-- MIDDLE SECTION: Changed Files (with resizer) -->
									<div
										class="changed-files-section"
										class:expand={!(assignedStackId || selectedFile)}
									>
										{@render commitChangedFiles(commitId!)}
									</div>

									<!-- BOTTOM SECTION: File Preview (no resizer) -->
									{#if assignedStackId || selectedFile}
										<ReduxResult projectId={stableProjectId} result={previewChangeQuery?.result}>
											{#snippet children(previewChange)}
												{@const diffQuery = diffService.getDiff(stableProjectId, previewChange)}
												{@const diffData = diffQuery.response}

												<div class="file-preview-section">
													{#if assignedStackId}
														<ConfigurableScrollableContainer
															zIndex="var(--z-lifted)"
															bind:viewport={selectionPreviewScrollContainer}
														>
															{@render assignedChangePreview(assignedStackId)}
														</ConfigurableScrollableContainer>
													{:else if selectedFile}
														<Drawer
															persistId="file-preview-drawer-{stableStackId}"
															bottomBorder={false}
														>
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
							</Dropzone>
						{:else}
							<div class="details-view__inner">
								<!-- TOP SECTION: Branch/Commit Details (no resizer) -->
								{#if branchName}
									{@render branchView(branchName)}
								{/if}

								<!-- MIDDLE SECTION: Changed Files (with resizer) -->
								<div
									class="changed-files-section"
									class:expand={!(assignedStackId || selectedFile)}
								>
									{#if branchName}
										{@render branchChangedFiles(branchName)}
									{/if}
								</div>

								<!-- BOTTOM SECTION: File Preview (no resizer) -->
								{#if assignedStackId || selectedFile}
									<ReduxResult projectId={stableProjectId} result={previewChangeQuery?.result}>
										{#snippet children(previewChange)}
											{@const diffQuery = diffService.getDiff(stableProjectId, previewChange)}
											{@const diffData = diffQuery.response}

											<div class="file-preview-section">
												{#if assignedStackId}
													<ConfigurableScrollableContainer
														zIndex="var(--z-lifted)"
														bind:viewport={selectionPreviewScrollContainer}
													>
														{@render assignedChangePreview(assignedStackId)}
													</ConfigurableScrollableContainer>
												{:else if selectedFile}
													<Drawer
														persistId="file-preview-drawer-{stableStackId}"
														bottomBorder={false}
													>
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
						{/if}
					{/if}
				</div>

				<!-- DETAILS VIEW WIDTH RESIZER - Only show when details view is open -->
				{#if compactDiv}
					<Resizer
						viewport={compactDiv}
						persistId="resizer-panel2-${stableStackId}"
						direction="right"
						showBorder
						minWidth={RESIZER_CONFIG.panel2.minWidth}
						maxWidth={RESIZER_CONFIG.panel2.maxWidth}
						defaultValue={RESIZER_CONFIG.panel2.defaultValue}
						syncName="panel2"
						onWidth={updateDetailsViewWidth}
					/>
				{/if}
			{/if}
		{/snippet}
	</ReduxResult>
</div>

<ReduxResult result={mcpConfigQuery.result} {projectId} {stackId} hideError>
	{#snippet children(mcpConfig, { stackId })}
		{@const laneState = stackId ? uiState.lane(stackId) : undefined}
		<CodegenMcpConfigModal
			disabledServers={laneState?.disabledMcpServers.current || []}
			bind:this={mcpConfigModal}
			{mcpConfig}
			toggleServer={(server) => {
				const disabledServers = laneState?.disabledMcpServers.current;
				if (disabledServers) {
					if (disabledServers.includes(server)) {
						laneState?.disabledMcpServers.set(disabledServers.filter((s) => s !== server));
					} else {
						laneState?.disabledMcpServers.set([...disabledServers, server]);
					}
				}
			}}
		/>
	{/snippet}
</ReduxResult>

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

		&:focus {
			outline: none;
		}
	}

	.stack-view {
		display: flex;
		position: relative;
		flex-shrink: 0;
		flex-direction: column;
		min-height: 100%;
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
		height: 100%;
		max-height: calc(100% - 24px);
		margin-right: 2px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
	}

	/* Needed for `focusCursor.svelte` to work correctly on `Drawer` components . */
	.details-view__inner {
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		overflow: hidden;
	}

	.changed-files-section {
		display: flex;
		flex-direction: column;
		overflow: hidden;

		&.expand {
			flex: 1;
			min-height: 0; /* Allow shrinking */
		}
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
</style>
