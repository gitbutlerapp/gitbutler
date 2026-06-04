<script lang="ts">
	import CommitContextMenu from "$components/commit/CommitContextMenu.svelte";
	import CommitDropIndicator from "$components/commit/CommitDropIndicator.svelte";
	import CommitListItem from "$components/commit/CommitListItem.svelte";
	import CommitPositionIndicator from "$components/commit/CommitPositionIndicator.svelte";
	import ChangedFilesPanel from "$components/files/ChangedFilesPanel.svelte";
	import { isLocalAndRemoteCommit, isUpstreamCommit } from "$components/lib";
	import Dropzone from "$components/shared/Dropzone.svelte";
	import DropzoneOverlay from "$components/shared/DropzoneOverlay.svelte";
	import LazyList from "$components/shared/LazyList.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import UpstreamActionRow from "$components/upstream/UpstreamActionRow.svelte";
	import UpstreamIntegrationActions from "$components/upstream/UpstreamIntegrationActions.svelte";
	import { commitCreatedAt } from "$lib/branches/v3";
	import { commitStatusLabel } from "$lib/commits/commit";
	import { findEarliestConflict } from "$lib/commits/utils";
	import { projectRunCommitHooks } from "$lib/config/config";
	import { draggableCommitV3 } from "$lib/dragging/draggable";
	import {
		AmendCommitWithChangeDzHandler,
		AmendCommitWithHunkDzHandler,
		CommitDropData,
		createCommitDropHandlers,
		type DzCommitData,
	} from "$lib/dragging/dropHandlers/commitDropHandler";
	import { DROPZONE_REGISTRY } from "$lib/dragging/registry";
	import {
		ReorderCommitDzFactory,
		ReorderCommitDzHandler,
	} from "$lib/dragging/stackingReorderDropzoneManager";
	import { commitUrl, FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
	import { IRC_API_SERVICE } from "$lib/irc/ircApiService";
	import { createCommitSelection } from "$lib/selection/key";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { getStackContext } from "$lib/stacks/stackController.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";

	import { UI_STATE, withStackBusy } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { TestId } from "@gitbutler/ui";
	import { DRAG_STATE_SERVICE } from "@gitbutler/ui/drag/dragStateService.svelte";
	import { focusable } from "@gitbutler/ui/focus/focusable";
	import { getTimeAgo } from "@gitbutler/ui/utils/timeAgo";
	import { isDefined } from "@gitbutler/ui/utils/typeguards";
	import type { Segment } from "@gitbutler/but-sdk";

	interface Props {
		branchName?: string;
		lastBranch: boolean;
		segment: Segment;
		stackingReorderDropzoneManager: ReorderCommitDzFactory;
		roundedTop?: boolean;
	}

	let { branchName, segment, lastBranch, stackingReorderDropzoneManager, roundedTop }: Props =
		$props();

	const controller = getStackContext();
	const stackService = inject(STACK_SERVICE);
	const forgeInfoService = inject(FORGE_INFO_SERVICE);
	const ircApiService = inject(IRC_API_SERVICE);
	const settingsService = inject(SETTINGS_SERVICE);
	const dropzoneRegistry = inject(DROPZONE_REGISTRY);
	const dragStateService = inject(DRAG_STATE_SERVICE);
	const uiState = inject(UI_STATE);

	const projectId = $derived(controller.projectId);
	const stackId = $derived(controller.stackId);
	const forgeInfoQuery = $derived(forgeInfoService.get(projectId));
	const forgeInfo = $derived(forgeInfoQuery.response);

	const settingsStore = settingsService.appSettings;
	const ircEnabled = $derived(
		($settingsStore?.featureFlags?.irc && $settingsStore?.irc?.connection?.enabled) ?? false,
	);

	const commitReactionsQuery = $derived(ircEnabled ? ircApiService.commitReactions() : undefined);
	const commitReactions = $derived(commitReactionsQuery?.response ?? {});

	const exclusiveAction = $derived(controller.exclusiveAction);
	const commitAction = $derived(exclusiveAction?.type === "commit" ? exclusiveAction : undefined);
	const selection = $derived(controller.selection);
	const runHooks = $derived(projectRunCommitHooks(projectId));

	const selectedBranchName = $derived(selection.current?.branchName);
	const selectedCommitId = $derived(selection.current?.commitId);

	const localAndRemoteCommits = $derived(segment.commits);
	const upstreamOnlyCommits = $derived(segment.commitsOnRemote);
	const hasRemoteCommits = $derived(upstreamOnlyCommits.length > 0);
	const hasCommits = $derived(localAndRemoteCommits.length > 0);

	/**
	 * Returns a flat ordered list of all commit IDs in this branch
	 * (upstream-only first, then local-and-remote), used for Shift+Click range selection.
	 */
	function getAllCommitIds(): string[] {
		const ups = upstreamOnlyCommits;
		const local = localAndRemoteCommits;
		return [...ups.map((c) => c.id), ...local.map((c) => c.id)];
	}

	async function handleCommitClick(commitId: string, upstream: boolean, event?: MouseEvent) {
		const currentSelection = controller.selection.current;
		const isMetaKey = event?.metaKey || event?.ctrlKey;
		const isShiftKey = event?.shiftKey;
		const isSameBranch = currentSelection?.branchName === branchName;

		if (isMetaKey) {
			// Cmd/Ctrl+Click: toggle this commit in/out of multi-selection
			if (!isSameBranch) {
				// Clicking in a different branch — start fresh selection here
				controller.selection.set({
					branchName,
					commitId,
					commitIds: [commitId],
					upstream,
					previewOpen: true,
				});
			} else {
				const currentIds = controller.selectedCommitIds;
				const alreadySelected = currentIds.includes(commitId);

				if (alreadySelected && currentIds.length === 1) {
					// Deselecting the only selected commit — clear selection
					controller.selection.set(undefined);
				} else if (alreadySelected) {
					const newIds = currentIds.filter((id) => id !== commitId);
					const newPrimary = newIds[newIds.length - 1]!;
					controller.selection.set({
						branchName,
						commitId: newPrimary,
						commitIds: newIds,
						upstream: currentSelection?.upstream ?? upstream,
						previewOpen: true,
					});
				} else {
					const newIds = [...currentIds, commitId];
					controller.selection.set({
						branchName,
						commitId,
						commitIds: newIds,
						upstream,
						previewOpen: true,
					});
				}
			}
		} else if (isShiftKey && currentSelection?.commitId && isSameBranch) {
			// Shift+Click: range select from primary commit to this one
			const allIds = getAllCommitIds();
			const anchorIdx = allIds.indexOf(currentSelection.commitId);
			const targetIdx = allIds.indexOf(commitId);

			if (anchorIdx !== -1 && targetIdx !== -1) {
				const start = Math.min(anchorIdx, targetIdx);
				const end = Math.max(anchorIdx, targetIdx);
				const rangeIds = allIds.slice(start, end + 1);
				controller.selection.set({
					branchName,
					commitId,
					commitIds: rangeIds,
					upstream,
					previewOpen: true,
				});
			} else {
				// Fallback: just select this commit
				controller.selection.set({ branchName, commitId, upstream, previewOpen: true });
			}
		} else {
			// Regular click: toggle single commit (same as before)
			if (currentSelection?.commitId === commitId && currentSelection?.branchName === branchName) {
				controller.selection.set(undefined);
			} else {
				controller.selection.set({ branchName, commitId, upstream, previewOpen: true });
			}
		}
		controller.clearWorktreeSelection();
	}

	async function handleUncommit(commitId: string) {
		await withStackBusy(
			uiState,
			projectId,
			{ commitId, stackIds: stackId ? [stackId] : undefined },
			async () => {
				await stackService.uncommit({
					projectId,
					stackId,
					commitIds: [commitId],
				});
			},
		);
	}

	async function handleUncommitSelected(selectedIds?: string[]) {
		const commitIds = selectedIds ?? controller.selectedCommitIds;
		if (commitIds.length === 0) return;

		// Filter to only IDs present in this branch to avoid invalid operations.
		const allIds = getAllCommitIds();
		const filtered = commitIds.filter((id) => allIds.includes(id));
		if (filtered.length === 0) return;

		await withStackBusy(
			uiState,
			projectId,
			{ stackIds: stackId ? [stackId] : undefined },
			async () => {
				await stackService.uncommit({
					projectId,
					stackId,
					commitIds: filtered,
				});
			},
		);
		controller.selection.set(undefined);
	}

	async function handleSquashSelected(selectedIds?: string[]) {
		const commitIds = selectedIds ?? controller.selectedCommitIds;

		// Filter to only IDs present in this branch to avoid invalid operations.
		const allIds = getAllCommitIds();
		const sorted = commitIds
			.filter((id) => allIds.includes(id))
			.sort((a, b) => allIds.indexOf(a) - allIds.indexOf(b));
		if (sorted.length < 2) return;
		const targetCommitId = sorted[sorted.length - 1]!;
		const sourceCommitIds = sorted.slice(0, -1);

		await withStackBusy(
			uiState,
			projectId,
			{ stackIds: stackId ? [stackId] : undefined },
			async () => {
				await stackService.squashCommits({
					projectId,
					sourceCommitIds,
					targetCommitId,
				});
			},
		);
		controller.selection.set(undefined);
	}

	/**
	 * When a commit is part of a multi-selection, build the allCommits array
	 * for drag data. Returns undefined for single-commit drags.
	 */
	function getDragAllCommits(commitId: string): DzCommitData[] | undefined {
		const selectedIds = controller.selectedCommitIds;
		if (selectedIds.length <= 1 || !selectedIds.includes(commitId)) return undefined;

		const allCommits = localAndRemoteCommits;
		return selectedIds
			.map((id) => {
				const c = allCommits.find((c) => c.id === id);
				if (!c) return undefined;
				return {
					id: c.id,
					isRemote: isUpstreamCommit(c),
					isIntegrated: isLocalAndRemoteCommit(c) && c.state.type === "Integrated",
					hasConflicts: isLocalAndRemoteCommit(c) && c.hasConflicts,
				} satisfies DzCommitData;
			})
			.filter((c): c is DzCommitData => c !== undefined);
	}

	function startEditingCommitMessage(commitId: string) {
		if (!branchName) return;
		controller.selection.set({ branchName, commitId, previewOpen: true });
		controller.projectState.exclusiveAction.set({
			type: "edit-commit-message",
			stackId,
			branchName,
			commitId,
		});
	}
</script>

{#snippet commitReorderDz(dropzone: ReorderCommitDzHandler)}
	{#if !controller.isCommitting}
		<Dropzone handlers={[dropzone]}>
			{#snippet overlay({ hovered, activated })}
				<CommitDropIndicator {hovered} {activated} />
			{/snippet}
		</Dropzone>
	{/if}
{/snippet}

<!-- {@render commitReorderDz(stackingReorderDropzoneManager.top(branchName))} -->

{#if hasCommits || hasRemoteCommits}
	<div
		class="commit-list hide-when-empty"
		class:rounded={roundedTop}
		use:focusable={{ vertical: true }}
	>
		{#if hasRemoteCommits}
			{#each upstreamOnlyCommits as commit, i (commit.id)}
				{@const first = i === 0}
				{@const lastCommit = i === upstreamOnlyCommits.length - 1}
				{@const selected =
					controller.isCommitSelected(commit.id) && branchName === selectedBranchName}
				{@const commitId = commit.id}
				{#if !controller.isCommitting}
					<CommitListItem
						type="Remote"
						{stackId}
						{commitId}
						commitMessage={commit.message}
						createdAt={commitCreatedAt(commit)}
						tooltip="Upstream"
						{branchName}
						{first}
						{lastCommit}
						{selected}
						active={controller.active && commit.id === selectedCommitId}
						reactions={commitReactions[commit.id]}
						onclick={(e) => handleCommitClick(commit.id, true, e)}
						disableCommitActions={false}
						editable={!!stackId}
					/>
				{/if}
			{/each}

			<UpstreamActionRow testId={TestId.UpstreamCommitsCommitAction} isLast={!hasCommits}>
				{#snippet action()}
					<h3 class="text-13 text-semibold m-b-4">Upstream has new commits</h3>
					<p class="text-12 text-body clr-text-2 m-b-14">Update your branch to stay current.</p>
					{#if branchName}
						<UpstreamIntegrationActions {projectId} {stackId} {branchName} />
					{/if}
				{/snippet}
			</UpstreamActionRow>
		{/if}

		{#if branchName}
			{@render commitReorderDz(stackingReorderDropzoneManager.top(branchName))}
		{/if}

		<LazyList items={localAndRemoteCommits} chunkSize={100}>
			{#snippet template(commit, { first, last })}
				{@const commitId = commit.id}
				{@const selected =
					controller.isCommitSelected(commit.id) && branchName === selectedBranchName}
				{#if controller.isCommitting}
					<!-- Only commits to the base can be `last`, see next `CommitPositionIndicator`. -->
					<CommitPositionIndicator
						{commitId}
						selected={(commitAction?.parentCommitId === commitId ||
							(first && commitAction?.parentCommitId === undefined)) &&
							!commitAction?.insertBelow &&
							commitAction?.branchName === branchName}
						{first}
						last={false}
						onclick={() => {
							if (!branchName) return;
							controller.projectState.exclusiveAction.set({
								type: "commit",
								stackId,
								branchName,
								parentCommitId: commitId,
							});
						}}
					/>
				{/if}
				{@const dzCommit: DzCommitData = {
							id: commit.id,
							isRemote: isUpstreamCommit(commit),
							isIntegrated: isLocalAndRemoteCommit(commit) && commit.state.type === 'Integrated',
							hasConflicts: isLocalAndRemoteCommit(commit) && commit.hasConflicts
						}}
				{@const { amendHandler, squashHandler, hunkHandler } = createCommitDropHandlers({
					projectId,
					stackId,
					commit: dzCommit,
					runHooks: $runHooks,
					okWithForce: true,
					onCommitIdChange: (newId) => {
						const wasSelected = controller.selection.current?.commitId === commitId;
						if (stackId && wasSelected) {
							const previewOpen = selection.current?.previewOpen ?? false;
							controller.laneState.selection.set({ branchName, commitId: newId, previewOpen });
						}
					},
				})}
				{@const tooltip = commitStatusLabel(commit.state.type)}
				<Dropzone
					handlers={branchName ? [amendHandler, squashHandler, hunkHandler].filter(isDefined) : []}
				>
					{#snippet overlay({ hovered, activated, handler })}
						{@const label =
							handler instanceof AmendCommitWithChangeDzHandler ||
							handler instanceof AmendCommitWithHunkDzHandler
								? "Amend"
								: "Squash"}
						<DropzoneOverlay {hovered} {activated} {label} />
					{/snippet}
					<div
						data-remove-from-panning
						use:draggableCommitV3={{
							disabled: false,
							label: commit.message.split("\n")[0],
							sha: commit.id.slice(0, 7),
							date: getTimeAgo(commitCreatedAt(commit)),
							authorImgUrl: undefined,
							commitType: commit.state.type,
							data:
								stackId && branchName
									? new CommitDropData(
											stackId,
											{
												id: commitId,
												isRemote: !!segment.remoteTrackingRefName,
												hasConflicts: isLocalAndRemoteCommit(commit) && commit.hasConflicts,
												isIntegrated:
													isLocalAndRemoteCommit(commit) && commit.state.type === "Integrated",
											},
											false,
											branchName,
											getDragAllCommits(commitId),
										)
									: undefined,
							dropzoneRegistry,
							dragStateService,
						}}
					>
						<CommitListItem
							commitId={commit.id}
							commitMessage={commit.message}
							type={commit.state.type}
							hasConflicts={commit.hasConflicts}
							busy={controller.busyCommitId === commit.id}
							diverged={commit.state.type === "LocalAndRemote" &&
								commit.id !== commit.state.subject}
							createdAt={commitCreatedAt(commit)}
							gerritReviewUrl={commit.gerritReviewUrl ?? undefined}
							{stackId}
							{branchName}
							{first}
							lastCommit={last}
							{lastBranch}
							{selected}
							expandChangedFiles={commit.id === selectedCommitId &&
								branchName === selectedBranchName &&
								controller.selectedCommitIds.length <= 1}
							{tooltip}
							active={controller.active && commit.id === selectedCommitId}
							reactions={commitReactions[commit.id]}
							onclick={(e) => handleCommitClick(commit.id, false, e)}
							disableCommitActions={false}
							editable={!!stackId}
						>
							{#snippet menu({ rightClickTrigger })}
								{@const selectedIds = controller.selectedCommitIds}
								{@const isMultiSelect =
									selectedIds.length > 1 &&
									branchName === selectedBranchName &&
									selectedIds.includes(commitId)}
								{@const data = {
									stackId,
									commitId,
									commitMessage: commit.message,
									commitStatus: commit.state.type,
									commitUrl: forgeInfo ? commitUrl(forgeInfo, commitId) : undefined,
									onUncommitClick: () => handleUncommit(commit.id),
									onEditMessageClick: () => startEditingCommitMessage(commit.id),
									multiSelect: isMultiSelect
										? {
												commitIds: selectedIds,
												onSquashSelected: () => handleSquashSelected(selectedIds),
												onUncommitSelected: () => handleUncommitSelected(selectedIds),
											}
										: undefined,
								}}
								<CommitContextMenu showOnHover {projectId} {rightClickTrigger} contextData={data} />
							{/snippet}

							{#snippet changedFiles()}
								{@const changesQuery = stackService.commitChanges(projectId, commitId)}

								<ReduxResult {projectId} {stackId} result={changesQuery.result}>
									{#snippet children(changesResult)}
										{@const commitsQuery = stackId ? localAndRemoteCommits : []}
										{@const commits = commitsQuery || []}
										{@const firstConflictedCommitId = findEarliestConflict(commits)?.id}

										<ChangedFilesPanel
											title="Changed files"
											{projectId}
											{stackId}
											visibleRange={controller.visibleRange}
											draggableFiles
											selectionId={createCommitSelection({ commitId: commitId, stackId })}
											persistId={`commit-${commitId}`}
											changes={changesResult.changes.filter(
												(change) =>
													!(change.path in (changesResult.conflictEntries?.entries ?? {})),
											)}
											stats={changesResult.stats ?? undefined}
											conflictEntries={changesResult.conflictEntries}
											ancestorMostConflictedCommitId={firstConflictedCommitId}
											autoselect
											allowUnselect={false}
											onFileClick={(index) => {
												// Ensure the commit is selected so the preview shows it
												const currentSelection = controller.selection.current;
												if (
													currentSelection?.commitId !== commitId ||
													currentSelection?.branchName !== branchName
												) {
													controller.selection.set({
														branchName,
														commitId,
														upstream: false,
														previewOpen: true,
													});
												}
												controller.jumpToIndex(index);
											}}
										/>
									{/snippet}
								</ReduxResult>
							{/snippet}
						</CommitListItem>
					</div>
				</Dropzone>
				{#if branchName}
					{@render commitReorderDz(
						stackingReorderDropzoneManager.belowCommit(branchName, commit.id),
					)}
				{/if}
				{#if controller.isCommitting && last}
					<CommitPositionIndicator
						commitId={commit.id}
						{first}
						{last}
						selected={exclusiveAction?.type === "commit" &&
							exclusiveAction.parentCommitId === commit.id &&
							exclusiveAction.insertBelow === true &&
							commitAction?.branchName === branchName}
						onclick={() => {
							if (!branchName) return;
							controller.projectState.exclusiveAction.set({
								type: "commit",
								stackId,
								branchName,
								parentCommitId: commit.id,
								insertBelow: true,
							});
						}}
					/>
				{/if}
			{/snippet}
		</LazyList>
	</div>
{/if}

<style lang="postcss">
	.commit-list {
		display: flex;
		position: relative;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: 0 0 var(--radius-ml) var(--radius-ml);
		background-color: var(--bg-1);

		&.rounded {
			border-radius: var(--radius-ml);
		}
	}
</style>
