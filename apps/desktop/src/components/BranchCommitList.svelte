<script lang="ts">
	import CardOverlay from "$components/CardOverlay.svelte";
	import CommitContextMenu from "$components/CommitContextMenu.svelte";
	import CommitGoesHere from "$components/CommitGoesHere.svelte";
	import CommitLineOverlay from "$components/CommitLineOverlay.svelte";
	import CommitRow from "$components/CommitRow.svelte";
	import Dropzone from "$components/Dropzone.svelte";
	import LazyList from "$components/LazyList.svelte";
	import NestedChangedFiles from "$components/NestedChangedFiles.svelte";
	import ReduxResult from "$components/ReduxResult.svelte";
	import UpstreamCommitsAction from "$components/UpstreamCommitsAction.svelte";
	import UpstreamIntegrationActions from "$components/UpstreamIntegrationActions.svelte";
	import { isLocalAndRemoteCommit, isUpstreamCommit } from "$components/lib";
	import { commitCreatedAt } from "$lib/branches/v3";
	import { commitStatusLabel } from "$lib/commits/commit";
	import {
		AmendCommitWithChangeDzHandler,
		AmendCommitWithHunkDzHandler,
		CommitDropData,
		createCommitDropHandlers,
		type DzCommitData,
	} from "$lib/commits/dropHandler";
	import { findEarliestConflict } from "$lib/commits/utils";
	import { projectRunCommitHooks } from "$lib/config/config";
	import { draggableCommitV3 } from "$lib/dragging/draggable";
	import { DROPZONE_REGISTRY } from "$lib/dragging/registry";
	import {
		ReorderCommitDzFactory,
		ReorderCommitDzHandler,
	} from "$lib/dragging/stackingReorderDropzoneManager";
	import { DEFAULT_FORGE_FACTORY } from "$lib/forge/forgeFactory.svelte";
	import { IRC_API_SERVICE } from "$lib/irc/ircApiService";
	import { createCommitSelection } from "$lib/selection/key";
	import { getStackContext } from "$lib/stack/stackController.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";

	import { combineResults } from "$lib/state/helpers";
	import { ensureValue } from "$lib/utils/validation";
	import { inject } from "@gitbutler/core/context";
	import { TestId } from "@gitbutler/ui";
	import { DRAG_STATE_SERVICE } from "@gitbutler/ui/drag/dragStateService.svelte";
	import { focusable } from "@gitbutler/ui/focus/focusable";
	import { getTimeAgo } from "@gitbutler/ui/utils/timeAgo";
	import { isDefined } from "@gitbutler/ui/utils/typeguards";
	import type { BranchDetails } from "$lib/stacks/stack";

	interface Props {
		branchName: string;
		lastBranch: boolean;
		branchDetails: BranchDetails;
		stackingReorderDropzoneManager: ReorderCommitDzFactory;
		roundedTop?: boolean;
	}

	let { branchName, branchDetails, lastBranch, stackingReorderDropzoneManager, roundedTop }: Props =
		$props();

	const controller = getStackContext();
	const stackService = inject(STACK_SERVICE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const ircApiService = inject(IRC_API_SERVICE);
	const dropzoneRegistry = inject(DROPZONE_REGISTRY);
	const dragStateService = inject(DRAG_STATE_SERVICE);

	const projectId = $derived(controller.projectId);
	const stackId = $derived(controller.stackId);

	const commitReactionsQuery = $derived(ircApiService.commitReactions());
	const commitReactions = $derived(commitReactionsQuery?.response ?? {});

	const exclusiveAction = $derived(controller.exclusiveAction);
	const commitAction = $derived(exclusiveAction?.type === "commit" ? exclusiveAction : undefined);
	const selection = $derived(controller.selection);
	const runHooks = $derived(projectRunCommitHooks(projectId));

	const selectedBranchName = $derived(selection.current?.branchName);
	const selectedCommitId = $derived(selection.current?.commitId);

	const localAndRemoteCommits = $derived(stackService.commits(projectId, stackId, branchName));
	const upstreamOnlyCommits = $derived(
		stackService.upstreamCommits(projectId, stackId, branchName),
	);

	async function handleCommitClick(commitId: string, upstream: boolean) {
		const currentSelection = controller.selection.current;
		// Toggle: if this exact commit is already selected, clear the selection
		if (currentSelection?.commitId === commitId && currentSelection?.branchName === branchName) {
			controller.selection.set(undefined);
		} else {
			controller.selection.set({ branchName, commitId, upstream, previewOpen: true });
		}
		controller.clearWorktreeSelection();
	}

	async function handleUncommit(commitId: string) {
		await stackService.uncommit({
			projectId,
			stackId: ensureValue(stackId),
			branchName,
			commitId,
		});
	}

	function startEditingCommitMessage(commitId: string) {
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
				<CommitLineOverlay {hovered} {activated} />
			{/snippet}
		</Dropzone>
	{/if}
{/snippet}

<ReduxResult
	{stackId}
	{projectId}
	result={combineResults(upstreamOnlyCommits.result, localAndRemoteCommits.result)}
>
	{#snippet children([upstreamOnlyCommits, localAndRemoteCommits], { stackId })}
		{@const hasRemoteCommits = upstreamOnlyCommits.length > 0}
		{@const hasCommits = localAndRemoteCommits.length > 0}

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
						{@const selected = commit.id === selectedCommitId && branchName === selectedBranchName}
						{@const commitId = commit.id}
						{#if !controller.isCommitting}
							<CommitRow
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
								active={controller.active}
								reactions={commitReactions[commit.id]}
								onclick={() => handleCommitClick(commit.id, true)}
								disableCommitActions={false}
								editable={!!stackId}
							/>
						{/if}
					{/each}

					<UpstreamCommitsAction testId={TestId.UpstreamCommitsCommitAction} isLast={!hasCommits}>
						{#snippet action()}
							<h3 class="text-13 text-semibold m-b-4">Upstream has new commits</h3>
							<p class="text-12 text-body clr-text-2 m-b-14">Update your branch to stay current.</p>
							<UpstreamIntegrationActions {projectId} {stackId} {branchName} />
						{/snippet}
					</UpstreamCommitsAction>
				{/if}

				{@render commitReorderDz(stackingReorderDropzoneManager.top(branchName))}

				<LazyList items={localAndRemoteCommits} chunkSize={100}>
					{#snippet template(commit, { first, last })}
						{@const commitId = commit.id}
						{@const selected = commit.id === selectedCommitId && branchName === selectedBranchName}
						{#if controller.isCommitting}
							<!-- Only commits to the base can be `last`, see next `CommitGoesHere`. -->
							<CommitGoesHere
								{commitId}
								selected={(commitAction?.parentCommitId === commitId ||
									(first && commitAction?.parentCommitId === undefined)) &&
									commitAction?.branchName === branchName}
								{first}
								last={false}
								onclick={() => {
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
						<Dropzone handlers={[amendHandler, squashHandler, hunkHandler].filter(isDefined)}>
							{#snippet overlay({ hovered, activated, handler })}
								{@const label =
									handler instanceof AmendCommitWithChangeDzHandler ||
									handler instanceof AmendCommitWithHunkDzHandler
										? "Amend"
										: "Squash"}
								<CardOverlay {hovered} {activated} {label} />
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
									data: stackId
										? new CommitDropData(
												stackId,
												{
													id: commitId,
													isRemote: !!branchDetails.remoteTrackingBranch,
													hasConflicts: isLocalAndRemoteCommit(commit) && commit.hasConflicts,
													isIntegrated:
														isLocalAndRemoteCommit(commit) && commit.state.type === "Integrated",
												},
												false,
												branchName,
											)
										: undefined,
									dropzoneRegistry,
									dragStateService,
								}}
							>
								<CommitRow
									commitId={commit.id}
									commitMessage={commit.message}
									type={commit.state.type}
									hasConflicts={commit.hasConflicts}
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
									{tooltip}
									active={controller.active}
									reactions={commitReactions[commit.id]}
									onclick={() => handleCommitClick(commit.id, false)}
									disableCommitActions={false}
									editable={!!stackId}
								>
									{#snippet menu({ rightClickTrigger })}
										{@const data = {
											stackId,
											commitId,
											commitMessage: commit.message,
											commitStatus: commit.state.type,
											commitUrl: forge.current.commitUrl(commitId),
											onUncommitClick: () => handleUncommit(commit.id),
											onEditMessageClick: () => startEditingCommitMessage(commit.id),
										}}
										<CommitContextMenu
											showOnHover
											{projectId}
											{rightClickTrigger}
											contextData={data}
										/>
									{/snippet}

									{#snippet changedFiles()}
										{@const changesQuery = stackService.commitChanges(projectId, commitId)}

										<ReduxResult {projectId} {stackId} result={changesQuery.result}>
											{#snippet children(changesResult)}
												{@const commitsQuery = stackId
													? stackService.commits(projectId, stackId, branchName)
													: undefined}
												{@const commits = commitsQuery?.response || []}
												{@const firstConflictedCommitId = findEarliestConflict(commits)?.id}

												<NestedChangedFiles
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
													stats={changesResult.stats}
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
								</CommitRow>
							</div>
						</Dropzone>
						{@render commitReorderDz(
							stackingReorderDropzoneManager.belowCommit(branchName, commit.id),
						)}
						{#if controller.isCommitting && last}
							<CommitGoesHere
								commitId={branchDetails.baseCommit}
								{first}
								{last}
								selected={exclusiveAction?.type === "commit" &&
									exclusiveAction.parentCommitId === branchDetails.baseCommit &&
									commitAction?.branchName === branchName}
								onclick={() => {
									controller.projectState.exclusiveAction.set({
										type: "commit",
										stackId,
										branchName,
										parentCommitId: branchDetails.baseCommit,
									});
								}}
							/>
						{/if}
					{/snippet}
				</LazyList>
			</div>
		{/if}
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.commit-list {
		display: flex;
		position: relative;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: 0 0 var(--radius-ml) var(--radius-ml);
		background-color: var(--clr-bg-1);

		&.rounded {
			border-radius: var(--radius-ml);
		}
	}
</style>
