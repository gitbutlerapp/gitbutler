<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import AsyncRender from '$components/v3/AsyncRender.svelte';
	import BranchList from '$components/v3/BranchList.svelte';
	import BranchView from '$components/v3/BranchView.svelte';
	import ChangedFiles from '$components/v3/ChangedFiles.svelte';
	import CommitView from '$components/v3/CommitView.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
	import NewCommitView from '$components/v3/NewCommitView.svelte';
	import SelectionView from '$components/v3/SelectionView.svelte';
	import WorktreeChanges from '$components/v3/WorktreeChanges.svelte';
	import { compactWorkspace } from '$lib/config/uiFeatureFlags';
	import { isParsedError } from '$lib/error/parser';
	import { DefinedFocusable } from '$lib/focus/focusManager.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import {
		IntelligentScrollingService,
		scrollingAttachment
	} from '$lib/intelligentScrolling/service';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { readKey, type SelectionId } from '$lib/selection/key';
	import { UncommittedService } from '$lib/selection/uncommittedService.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { computeChangeStatus } from '$lib/utils/fileStatus';
	import { inject } from '@gitbutler/shared/context';
	import { persistWithExpiration } from '@gitbutler/shared/persisted';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import FileViewHeader from '@gitbutler/ui/file/FileViewHeader.svelte';
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

	const [
		uiState,
		uncommittedService,
		idSelection,
		stackService,
		intelligentScrollingService,
		diffService
	] = inject(
		UiState,
		UncommittedService,
		IdSelection,
		StackService,
		IntelligentScrollingService,
		DiffService
	);
	const projectState = $derived(uiState.project(projectId));

	const action = $derived(projectState.exclusiveAction.current);
	const isCommitting = $derived(action?.type === 'commit' && action.stackId === stack.id);

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

	const previewKey = $derived(assignedKey || selectedKey);
	const previewChangeResult = $derived(
		previewKey ? idSelection.changeByKey(projectId, previewKey) : undefined
	);

	const changes = $derived(uncommittedService.changesByStackId(stack.id || null));

	let stackViewEl = $state<HTMLDivElement>();
	let detailsEl = $state<HTMLDivElement>();
	let previewEl = $state<HTMLDivElement>();

	let compactDiv = $state<HTMLDivElement>();

	let verticalHeight = $state<number>(0);
	let verticalHeightRem = $derived(pxToRem(verticalHeight, 1));

	let actualDetailsHeight = $state<number>(0);
	let actualDetailsHeightRem = $derived(pxToRem(actualDetailsHeight, 1));

	let minDetailsHeight = $state(10);
	let minChangedFilesHeight = $state(10);
	let minPreviewHeight = $derived(previewChangeResult ? 10 : 0);

	let maxDetailsHeight = $derived(verticalHeightRem - minChangedFilesHeight - minPreviewHeight);
	let maxChangedFilesHeight = $derived(
		verticalHeightRem - actualDetailsHeightRem - minPreviewHeight
	);

	const defaultBranchResult = $derived(stackService.defaultBranch(projectId, stack.id));
	const defaultBranchName = $derived(defaultBranchResult?.current.data);

	function startCommit() {
		projectState.exclusiveAction.set({
			type: 'commit',
			branchName: defaultBranchName,
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

	// Clear selection if branch cannot be found.
	// TODO: How can we express this better?
	$effect(() => {
		if (selection.current && branchesResult.current.data) {
			setTimeout(() => {
				if (selection.current && branchesResult.current.data) {
					const { branchName } = selection.current;
					if (!branchesResult.current.data.some((b) => b.name === branchName)) {
						selection.set(undefined);
					}
				}
			}, 500);
		}
	});

	// Clear selection if commit cannot be found.
	// TODO: How can we express this better?
	$effect(() => {
		if (selection.current) {
			setTimeout(() => {
				if (selection.current) {
					const { branchName, commitId, upstream } = selection.current;
					if (branchName && commitId) {
						if (upstream) {
							stackService.fetchUpstreamCommitById(projectId, stack.id, commitId).then((result) => {
								if (!result.data) {
									selection.set(undefined);
								}
							});
						} else {
							stackService.fetchCommitById(projectId, stack.id, commitId).then((result) => {
								if (!result.data) {
									selection.set(undefined);
								}
							});
						}
					}
				}
			}, 500);
		}
	});

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
		testId={TestId.WorktreeSelectionView}
		{projectId}
		diffOnly={$compactWorkspace}
		topPadding={$compactWorkspace}
		selectionId={{ ...assignedKey, type: 'worktree', stackId }}
		onclose={() => {
			intelligentScrollingService.show(projectId, stack.id, 'stack');
		}}
	/>
{/snippet}

{#snippet otherChangePreview(selectionId: SelectionId)}
	<SelectionView
		testId={TestId.StackSelectionView}
		{projectId}
		{selectionId}
		diffOnly={$compactWorkspace}
		topPadding={$compactWorkspace}
		onclose={() => {
			intelligentScrollingService.show(projectId, stack.id, 'details');
		}}
	/>
{/snippet}

{#snippet branchView(branchName: string)}
	<BranchView
		collapsible
		stackId={stack.id}
		{projectId}
		{branchName}
		active={selectedKey?.type === 'branch' &&
			selectedKey.branchName === branchName &&
			focusedStackId === stack.id}
		scrollToType="details"
		scrollToId={stack.id}
		{onerror}
		{onclose}
	>
		{#snippet resizer({ element, collapsed })}
			{#if $compactWorkspace}
				<Resizer
					bind:clientHeight={actualDetailsHeight}
					viewport={element}
					passive={collapsed}
					direction="down"
					minHeight={minDetailsHeight}
					maxHeight={maxDetailsHeight}
				/>
			{/if}
		{/snippet}
	</BranchView>
{/snippet}

{#snippet commitView(branchName: string, commitId: string)}
	<CommitView
		{projectId}
		collapsible={$compactWorkspace}
		stackId={stack.id}
		commitKey={{
			stackId: stack.id,
			branchName,
			commitId,
			upstream: !!upstream
		}}
		draggableFiles
		active={selectedKey?.type === 'commit' && focusedStackId === stack.id}
		scrollToId={stack.id}
		scrollToType="details"
		{onerror}
		{onclose}
	>
		{#snippet resizer({ element, collapsed })}
			{#if $compactWorkspace}
				<Resizer
					bind:clientHeight={actualDetailsHeight}
					viewport={element}
					passive={collapsed}
					direction="down"
					minHeight={minDetailsHeight}
					maxHeight={maxDetailsHeight}
				/>
			{/if}
		{/snippet}
	</CommitView>
{/snippet}

{#snippet commitChangedFiles(commitId: string)}
	{@const active = selectedKey?.type === 'commit' && focusedStackId === stack.id}
	{@const changesResult = stackService.commitChanges(projectId, commitId)}
	<ReduxResult {projectId} stackId={stack.id} result={changesResult.current}>
		{#snippet children(changes, { projectId, stackId })}
			<ChangedFiles
				title="Changed Files"
				{projectId}
				{stackId}
				grow
				draggableFiles
				collapsible={$compactWorkspace}
				selectionId={{ type: 'commit', commitId }}
				changes={changes.changes.filter(
					(change) => !(change.path in (changes.conflictEntries?.entries ?? {}))
				)}
				conflictEntries={changes.conflictEntries}
				{active}
			>
				{#snippet resizer({ element, collapsed })}
					{#if $compactWorkspace}
						<Resizer
							viewport={element}
							maxHeight={maxChangedFilesHeight}
							minHeight={minChangedFilesHeight}
							passive={collapsed}
							direction="down"
							syncName="blah"
						/>
					{/if}
				{/snippet}
			</ChangedFiles>
		{/snippet}
	</ReduxResult>
{/snippet}

{#snippet branchChangedFiles(branchName: string)}
	{@const active = selectedKey?.type === 'branch'}
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
				collapsible={$compactWorkspace}
				selectionId={{ type: 'branch', stackId: stack.id, branchName }}
				{changes}
				{active}
			>
				{#snippet resizer({ element, collapsed })}
					{#if $compactWorkspace}
						<Resizer
							viewport={element}
							maxHeight={maxChangedFilesHeight}
							minHeight={minChangedFilesHeight}
							passive={collapsed}
							direction="down"
							syncName="blah"
						/>
					{/if}
				{/snippet}
			</ChangedFiles>
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
				minWidth={16}
				maxWidth={64}
				syncName="panel1"
				imitateBorder
			/>
			<ReduxResult {projectId} result={branchesResult.current}>
				{#snippet children(branches)}
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
													Drop files to assign to the lane
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
											disabled={defaultBranchResult?.current.isLoading}
											onclick={() => {
												startCommit();
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

		{#if $compactWorkspace && (commitId || branchName || assignedKey || selectedKey)}
			<div class="combined-view" bind:this={compactDiv} bind:clientHeight={verticalHeight}>
				{#if branchName && commitId}
					{@render commitView(branchName, commitId)}
					{@render commitChangedFiles(commitId)}
				{:else if branchName}
					{@render branchView(branchName)}
					{@render branchChangedFiles(branchName)}
				{/if}

				{#if assignedKey || selectedKey}
					<ReduxResult {projectId} result={previewChangeResult?.current}>
						{#snippet children(previewChange)}
							{@const diffResult = diffService.getDiff(projectId, previewChange)}
							{@const diffData = diffResult.current.data}
							<Drawer headerNoPaddingLeft collapsible>
								{#snippet header()}
									<FileViewHeader
										compact
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
								{#if assignedKey?.type === 'worktree' && assignedKey.stackId}
									{@render assignedChangePreview(assignedKey.stackId)}
								{:else if selectedKey}
									{@render otherChangePreview(selectedKey)}
								{/if}
							</Drawer>
						{/snippet}
					</ReduxResult>
				{/if}
				<!-- The id of this resizer is intentionally the same as in default view. -->
				<Resizer
					viewport={compactDiv}
					persistId="resizer-panel2-${stack.id}"
					direction="right"
					minWidth={16}
					maxWidth={56}
					defaultValue={20}
					syncName="panel2"
					imitateBorder
				/>
			</div>
		{:else if branchName}
			<div
				bind:this={detailsEl}
				style:width={uiState.global.detailsWidth.current + 'rem'}
				class="details"
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
				<Resizer
					viewport={detailsEl}
					persistId="resizer-panel2-${stack.id}"
					direction="right"
					minWidth={16}
					defaultValue={20}
					maxWidth={56}
					syncName="panel2"
					imitateBorder
				/>
			</div>
		{/if}
		{#if (assignedKey?.type === 'worktree' && assignedKey.stackId) || selectedKey}
			<div
				bind:this={previewEl}
				style:width={uiState.global.previewWidth.current + 'rem'}
				class="preview"
				data-remove-from-draggable
				{@attach scrollingAttachment(intelligentScrollingService, stack.id, 'diff')}
			>
				{#if assignedKey?.type === 'worktree' && assignedKey.stackId}
					{@render assignedChangePreview(assignedKey.stackId)}
				{:else if selectedKey}
					{@render otherChangePreview(selectedKey)}
				{/if}
				<Resizer
					viewport={previewEl}
					persistId="resizer-panel3-${stack.id}"
					direction="right"
					minWidth={20}
					maxWidth={96}
					syncName="panel3"
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

	.details,
	.preview {
		position: relative;
		flex-shrink: 0;
		height: 100%;
		white-space: wrap;
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
