<!--
	Compound child that renders the stack details/preview panel (right side).
	Reads shared state from StackController via context.

	Handles: commit view, branch view, worktree multi-diff, IRC channel.

	Usage:
	```svelte
	<StackDetails
		{ircChannel}
		onWidthChange={(w) => ...}
	/>
	```
-->
<script lang="ts">
	import CommitView from "$components/commit/CommitView.svelte";
	import MultiDiffView from "$components/diff/MultiDiffView.svelte";
	import IrcChannel from "$components/irc/IrcChannel.svelte";
	import { isLocalAndRemoteCommit, isUpstreamCommit } from "$components/lib";
	import Dropzone from "$components/shared/Dropzone.svelte";
	import DropzoneOverlay from "$components/shared/DropzoneOverlay.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import Resizer from "$components/shared/Resizer.svelte";
	import BranchView from "$components/views/BranchView.svelte";
	import { projectRunCommitHooks } from "$lib/config/config";
	import {
		AmendCommitWithChangeDzHandler,
		AmendCommitWithHunkDzHandler,
		createCommitDropHandlers,
		type DzCommitData,
	} from "$lib/dragging/dropHandlers/commitDropHandler";

	import { UNCOMMITTED_SERVICE } from "$lib/selection/uncommittedService.svelte";
	import { precomputeStack, segmentContext } from "$lib/stacks/segmentContext";
	import { getStackContext } from "$lib/stacks/stackController.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { TestId } from "@gitbutler/ui";
	import { focusable } from "@gitbutler/ui/focus/focusable";
	import { isDefined } from "@gitbutler/ui/utils/typeguards";
	import { get } from "svelte/store";
	import { fly } from "svelte/transition";
	import type { Segment } from "@gitbutler/but-sdk";
	type Props = {
		ircChannel?: string;
		segments: Segment[];
		onWidthChange: (width: number) => void;
	};

	const { ircChannel, segments, onWidthChange }: Props = $props();

	const controller = getStackContext();

	const stackService = inject(STACK_SERVICE);

	const uncommittedService = inject(UNCOMMITTED_SERVICE);

	const RESIZER_CONFIG = {
		minWidth: 20,
		maxWidth: 64,
		defaultValue: 37,
	};
	const DETAILS_RIGHT_PADDING_REM = 1;

	const selectedCommit = $derived.by(() => {
		const id = controller.commitId;
		if (!id) return undefined;
		for (const segment of segments) {
			const c = segment.commits.find((c) => c.id === id);
			if (c) return c;
			const uc = segment.commitsOnRemote.find((c) => c.id === id);
			if (uc) return uc;
		}
		return undefined;
	});
	const runHooks = $derived(projectRunCommitHooks(controller.projectId));
	const commitFiles = $derived(
		controller.commitId
			? stackService.commitChanges(controller.projectId, controller.commitId)
			: undefined,
	);
	const assignedFiles = $derived(
		uncommittedService.getChangesByStackId(controller.stackId || null),
	);

	let multiDiffView = $state<MultiDiffView>();

	$effect(() => {
		if (multiDiffView) {
			controller.registerDiffView({
				jump: (index) => multiDiffView?.jumpToIndex(index),
				popout: () => multiDiffView?.openFloatingDiff(),
			});
		}
		return () => controller.unregisterDiffView();
	});

	function onVisibleChange(change: { start: number; end: number } | undefined) {
		controller.setVisibleRange(change);
	}

	let detailsEl = $state<HTMLDivElement>();

	const projectId = $derived(controller.projectId);
	const stackId = $derived(controller.stackId);
	const laneId = $derived(controller.laneId);
	const branchName = $derived(controller.branchName);
	const commitId = $derived(controller.commitId);
	const focusedFileStore = $derived(controller.focusedFileStore);
	const selection = $derived(controller.laneState.selection.current);
	const selectedIndex = $derived(
		branchName ? segments.findIndex((s) => s.refName?.displayName === branchName) : -1,
	);
	const selectedSegment = $derived(selectedIndex >= 0 ? segments[selectedIndex] : undefined);
	const stackPrecomputed = $derived(precomputeStack(segments));
	const selectedContext = $derived(
		selectedIndex >= 0 ? segmentContext(segments, selectedIndex, stackPrecomputed) : undefined,
	);

	// If the selected branch is no longer in the stack (e.g. it was renamed
	// or deleted), clear the selection. This replaces the BranchNotFound
	// handling that used to live in BranchView's ReduxResult onerror.
	$effect(() => {
		if (branchName && !selectedSegment) {
			controller.selection.set(undefined);
		}
	});
</script>

<div
	in:fly={{ y: 20, duration: 200 }}
	class="details-view"
	class:irc-view={selection?.irc}
	bind:this={detailsEl}
	data-details={stackId}
	style:right="{DETAILS_RIGHT_PADDING_REM}rem"
	use:focusable={{ vertical: true }}
	data-testid={TestId.StackPreview}
>
	{#if stackId && selection?.irc && ircChannel}
		<IrcChannel projectId={controller.projectId} type="group" channel={ircChannel} autojoin />
	{:else}
		{@const commit = selectedCommit}
		{@const dzCommit: DzCommitData | undefined = commit
			? {
					id: commit.id,
					isRemote: isUpstreamCommit(commit),
					isIntegrated:
						isLocalAndRemoteCommit(commit) && commit.state.type === "Integrated",
					hasConflicts: isLocalAndRemoteCommit(commit) && commit.hasConflicts,
				}
			: undefined}
		{@const { amendHandler, squashHandler, hunkHandler } =
			controller.isCommitView && dzCommit
				? createCommitDropHandlers({
						projectId: controller.projectId,
						stackId: controller.stackId,
						commit: dzCommit,
						runHooks: $runHooks,
						okWithForce: true,
						onCommitIdChange: (newId) => {
							// branchName may be undefined for commits in anonymous segments
							// (refName === null); the selection state already tolerates that.
							if (selection) {
								const previewOpen = selection.previewOpen ?? true;
								controller.laneState.selection.set({
									branchName,
									commitId: newId,
									previewOpen,
								});
							}
						},
					})
				: { amendHandler: undefined, squashHandler: undefined, hunkHandler: undefined }}
		{#if commitId}
			<Dropzone
				handlers={[amendHandler, squashHandler, hunkHandler].filter(isDefined)}
				fillHeight
				overflow
			>
				{#snippet overlay({ hovered, activated, handler })}
					{@const label =
						handler instanceof AmendCommitWithChangeDzHandler ||
						handler instanceof AmendCommitWithHunkDzHandler
							? "Amend"
							: "Squash"}
					<DropzoneOverlay {hovered} {activated} {label} />
				{/snippet}
				<div class="details-view__inner">
					{#if commit}
						<CommitView
							{projectId}
							{stackId}
							{laneId}
							{commit}
							draggableFiles
							rounded
							onclose={() => controller.closePreview()}
							onpopout={() => controller.openFloatingDiff()}
						/>
					{/if}
					{#if commitFiles}
						{@const commitResult = commitFiles?.result}
						{#if commitResult}
							<ReduxResult {projectId} {stackId} result={commitResult}>
								{#snippet children(commit)}
									<MultiDiffView
										{stackId}
										selectionId={{ type: "commit", commitId, stackId }}
										bind:this={multiDiffView}
										{projectId}
										changes={commit.changes}
										draggable={true}
										selectable={false}
										startIndex={focusedFileStore ? get(focusedFileStore)?.index : undefined}
										{onVisibleChange}
									/>
								{/snippet}
							</ReduxResult>
						{/if}
					{/if}
				</div>
			</Dropzone>
		{:else if branchName && selectedSegment && selectedContext}
			{@const changesQuery = stackService.branchChanges({
				projectId: controller.projectId,
				branch: branchName,
			})}
			<div class="details-view__inner">
				<BranchView
					{stackId}
					{laneId}
					{projectId}
					{branchName}
					segment={selectedSegment}
					branchIndex={selectedContext.branchIndex}
					parent={selectedContext.parent}
					child={selectedContext.child}
					withForce={selectedContext.withForce}
					stackPrNumbers={selectedContext.stackPrNumbers}
					stackLength={segments.length}
					onclose={() => controller.closePreview()}
					rounded
					onpopout={() => controller.openFloatingDiff()}
				/>
				<ReduxResult {projectId} {stackId} result={changesQuery.result}>
					{#snippet children(result)}
						<MultiDiffView
							{stackId}
							selectionId={{
								type: "branch",
								branchName,
								remote: undefined,
								stackId,
							}}
							changes={result.changes}
							bind:this={multiDiffView}
							{projectId}
							draggable={true}
							selectable={false}
							startIndex={focusedFileStore ? get(focusedFileStore)?.index : undefined}
							{onVisibleChange}
						/>
					{/snippet}
				</ReduxResult>
			</div>
		{:else if focusedFileStore}
			<MultiDiffView
				{stackId}
				selectionId={{ type: "worktree", stackId }}
				changes={assignedFiles}
				bind:this={multiDiffView}
				{projectId}
				draggable={true}
				selectable={controller.isCommitting}
				onclose={() => controller.closePreview()}
				startIndex={focusedFileStore ? get(focusedFileStore)?.index : undefined}
				{onVisibleChange}
			/>
		{/if}
	{/if}
</div>

<!-- DETAILS VIEW WIDTH RESIZER -->
{#if detailsEl}
	<Resizer
		viewport={detailsEl}
		persistId="resizer-panel2-${stackId}"
		direction="right"
		edgeOffsetRem={DETAILS_RIGHT_PADDING_REM}
		fullLayerCrossAxis
		minWidth={RESIZER_CONFIG.minWidth}
		maxWidth={RESIZER_CONFIG.maxWidth}
		defaultValue={RESIZER_CONFIG.defaultValue}
		syncName="panel2"
		onWidth={onWidthChange}
	/>
{/if}

<style lang="postcss">
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
	}

	.irc-view {
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-ml);
		background-color: var(--bg-1);
	}

	:global(.details-view__inner) {
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		height: 100%;
		gap: 8px;
	}
</style>
