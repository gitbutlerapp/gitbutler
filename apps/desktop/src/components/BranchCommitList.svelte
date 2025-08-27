<script lang="ts">
	import CardOverlay from '$components/CardOverlay.svelte';
	import CommitAction from '$components/CommitAction.svelte';
	import CommitContextMenu from '$components/CommitContextMenu.svelte';
	import CommitGoesHere from '$components/CommitGoesHere.svelte';
	import CommitRow from '$components/CommitRow.svelte';
	import CommitsAccordion from '$components/CommitsAccordion.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import LineOverlay from '$components/LineOverlay.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { hasConflicts, isLocalAndRemoteCommit, isUpstreamCommit } from '$components/lib';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { commitStatusLabel } from '$lib/commits/commit';
	import {
		AmendCommitWithChangeDzHandler,
		AmendCommitWithHunkDzHandler,
		CommitDropData,
		type DzCommitData,
		SquashCommitDzHandler
	} from '$lib/commits/dropHandler';
	import { projectRunCommitHooks } from '$lib/config/config';
	import { DRAG_STATE_SERVICE } from '$lib/dragging/dragStateService.svelte';
	import { draggableCommitV3 } from '$lib/dragging/draggable';
	import { DROPZONE_REGISTRY } from '$lib/dragging/registry';
	import {
		ReorderCommitDzFactory,
		ReorderCommitDzHandler
	} from '$lib/dragging/stackingReorderDropzoneManager';
	import { DefinedFocusable } from '$lib/focus/focusManager';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { HOOKS_SERVICE } from '$lib/hooks/hooksService';
	import { STACK_SERVICE, type SeriesIntegrationStrategy } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { ensureValue } from '$lib/utils/validation';
	import { inject } from '@gitbutler/shared/context';
	import { Button, Modal, TestId } from '@gitbutler/ui';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import type { Commit } from '$lib/branches/v3';
	import type { CommitStatusType } from '$lib/commits/commit';
	import type { BranchDetails } from '$lib/stacks/stack';

	const integrationStrategies = {
		default: {
			label: 'Integrate upstream',
			style: 'warning',
			kind: 'solid',
			icon: undefined,
			action: () => integrate()
		},
		reset: {
			label: 'Reset to remote…',
			style: 'neutral',
			kind: 'outline',
			icon: 'warning-small',
			action: confirmReset
		}
	} as const;

	type IntegrationStrategy = keyof typeof integrationStrategies;

	interface Props {
		active: boolean;
		projectId: string;
		stackId?: string;
		laneId: string;
		branchName: string;
		firstBranch: boolean;
		lastBranch: boolean;
		branchDetails: BranchDetails;
		stackingReorderDropzoneManager: ReorderCommitDzFactory;

		handleUncommit: (commitId: string, branchName: string) => Promise<void>;
		startEditingCommitMessage: (branchName: string, commitId: string) => void;
		handleEditPatch: (args: {
			commitId: string;
			type: CommitStatusType;
			hasConflicts: boolean;
			isAncestorMostConflicted: boolean;
		}) => void;
		onselect?: () => void;
	}

	let {
		active,
		projectId,
		stackId,
		laneId,
		branchName,
		branchDetails,
		firstBranch,
		lastBranch,
		stackingReorderDropzoneManager,
		handleUncommit,
		startEditingCommitMessage,
		handleEditPatch,
		onselect
	}: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const uiState = inject(UI_STATE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const hooksService = inject(HOOKS_SERVICE);
	const dropzoneRegistry = inject(DROPZONE_REGISTRY);
	const dragStateService = inject(DRAG_STATE_SERVICE);
	const [integrateUpstreamCommits, upstreamIntegration] = stackService.integrateUpstreamCommits;

	const projectState = $derived(uiState.project(projectId));
	const exclusiveAction = $derived(projectState.exclusiveAction.current);
	const commitAction = $derived(exclusiveAction?.type === 'commit' ? exclusiveAction : undefined);
	const isCommitting = $derived(
		exclusiveAction?.type === 'commit' && exclusiveAction.stackId === stackId
	);
	const laneState = $derived(uiState.lane(laneId));
	const selection = $derived(laneState.selection);
	const runHooks = $derived(projectRunCommitHooks(projectId));

	const selectedBranchName = $derived(selection.current?.branchName);
	const selectedCommitId = $derived(selection.current?.commitId);

	const localAndRemoteCommits = $derived(stackService.commits(projectId, stackId, branchName));
	const upstreamOnlyCommits = $derived(
		stackService.upstreamCommits(projectId, stackId, branchName)
	);

	const baseBranchResponse = $derived(baseBranchService.baseBranch(projectId));
	const base = $derived(baseBranchResponse.current.data);
	const baseSha = $derived(base?.baseSha);

	let confirmResetModal = $state<ReturnType<typeof Modal>>();

	async function integrate(strategy?: SeriesIntegrationStrategy): Promise<void> {
		await integrateUpstreamCommits({
			projectId,
			stackId: ensureValue(stackId),
			seriesName: branchName,
			strategy
		});
	}

	function confirmReset() {
		confirmResetModal?.show();
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

	async function handleCommitClick(commitId: string, upstream: boolean) {
		if (selectedCommitId !== commitId) {
			laneState.selection.set({ branchName, commitId, upstream });
		}
		projectState.stackId.set(stackId);
		onselect?.();
	}
</script>

<Modal
	bind:this={confirmResetModal}
	title="Reset to remote"
	type="warning"
	width="small"
	onSubmit={async (close) => {
		await integrate('hardreset');
		close();
	}}
>
	<p class="text-13 text-body helper-text">
		This will reset the branch to the state of the remote branch. All local changes will be
		overwritten.
	</p>
	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<Button style="error" type="submit">Reset</Button>
	{/snippet}
</Modal>

{#snippet integrateUpstreamButton(strategy: IntegrationStrategy)}
	{@const { label, icon, style, kind, action } = integrationStrategies[strategy]}
	<Button
		testId={TestId.UpstreamCommitsIntegrateButton}
		{style}
		{kind}
		grow
		{icon}
		reversedDirection
		loading={upstreamIntegration.current.isLoading}
		onclick={action}
	>
		{label}
	</Button>
{/snippet}

{#snippet commitReorderDz(dropzone: ReorderCommitDzHandler)}
	{#if !isCommitting}
		<Dropzone handlers={[dropzone]}>
			{#snippet overlay({ hovered, activated })}
				<LineOverlay {hovered} {activated} />
			{/snippet}
		</Dropzone>
	{/if}
{/snippet}

<ReduxResult
	{stackId}
	{projectId}
	result={combineResults(upstreamOnlyCommits.current, localAndRemoteCommits.current)}
>
	{#snippet children([upstreamOnlyCommits, localAndRemoteCommits], { stackId })}
		{@const hasRemoteCommits = upstreamOnlyCommits.length > 0}
		{@const hasCommits = localAndRemoteCommits.length > 0}
		{@const ancestorMostConflicted = getAncestorMostConflicted(localAndRemoteCommits)}
		{@const thisIsTheRightBranch = firstBranch && selectedCommitId === undefined}

		{#if !hasCommits && isCommitting && thisIsTheRightBranch}
			<CommitGoesHere
				commitId={baseSha}
				last
				draft
				selected={branchName === commitAction?.branchName ||
					!commitAction?.branchName ||
					!commitAction.parentCommitId}
				onclick={() => {
					projectState.exclusiveAction.set({
						type: 'commit',
						stackId,
						branchName,
						parentCommitId: branchDetails.baseCommit
					});
				}}
			/>
		{/if}

		{@render commitReorderDz(stackingReorderDropzoneManager.top(branchName))}

		{#if hasCommits || hasRemoteCommits}
			<div
				class="commit-list hide-when-empty"
				use:focusable={{
					id: DefinedFocusable.CommitList,
					list: true,
					disabled: localAndRemoteCommits.length <= 1
				}}
			>
				{#if hasRemoteCommits}
					<CommitsAccordion
						testId={TestId.UpstreamCommitsAccordion}
						count={Math.min(upstreamOnlyCommits.length, 3)}
						isLast={!hasCommits}
						type="upstream"
						displayHeader={upstreamOnlyCommits.length > 1}
					>
						{#snippet title()}
							<span class="text-13 text-body text-semibold">Upstream commits</span>
						{/snippet}

						{#each upstreamOnlyCommits as commit, i (commit.id)}
							{@const first = i === 0}
							{@const lastCommit = i === upstreamOnlyCommits.length - 1}
							{@const selected =
								commit.id === selectedCommitId && branchName === selectedBranchName}
							{@const commitId = commit.id}
							{#if !isCommitting}
								<CommitRow
									type="Remote"
									{stackId}
									{commitId}
									commitMessage={commit.message}
									createdAt={commit.createdAt}
									tooltip="Upstream"
									{branchName}
									{first}
									{lastCommit}
									{selected}
									{active}
									onclick={() => handleCommitClick(commit.id, true)}
									disableCommitActions={false}
								/>
							{/if}
						{/each}

						<CommitAction type="Remote" isLast={!hasCommits}>
							{#snippet action()}
								<!-- TODO: Ability to select other actions would be nice -->
								{@render integrateUpstreamButton('default')}
							{/snippet}
						</CommitAction>
					</CommitsAccordion>
				{/if}

				{#each localAndRemoteCommits as commit, i (commit.id)}
					{@const first = i === 0}
					{@const last = i === localAndRemoteCommits.length - 1}
					{@const commitId = commit.id}
					{@const selected = commit.id === selectedCommitId && branchName === selectedBranchName}
					{#if isCommitting}
						<!-- Only commits to the base can be `last`, see next `CommitGoesHere`. -->
						<CommitGoesHere
							{commitId}
							selected={(commitAction?.parentCommitId === commitId ||
								(first && commitAction?.parentCommitId === undefined)) &&
								commitAction?.branchName === branchName}
							{first}
							last={false}
							onclick={() => {
								projectState.exclusiveAction.set({
									type: 'commit',
									stackId,
									branchName,
									parentCommitId: commitId
								});
							}}
						/>
					{/if}
					{@const dzCommit: DzCommitData = {
						id: commit.id,
						isRemote: isUpstreamCommit(commit),
						isIntegrated: isLocalAndRemoteCommit(commit) && commit.state.type === 'Integrated',
						hasConflicts: isLocalAndRemoteCommit(commit) && commit.hasConflicts,
					}}
					{@const amendHandler = stackId
						? new AmendCommitWithChangeDzHandler(
								projectId,
								stackService,
								hooksService,
								stackId,
								$runHooks,
								dzCommit,
								(newId) => uiState.lane(stackId).selection.set({ branchName, commitId: newId }),
								uiState
							)
						: undefined}
					{@const squashHandler = stackId
						? new SquashCommitDzHandler({
								stackService,
								projectId,
								stackId,
								commit: dzCommit
							})
						: undefined}
					{@const hunkHandler = stackId
						? new AmendCommitWithHunkDzHandler({
								stackService,
								hooksService,
								projectId,
								stackId,
								commit: dzCommit,
								runHooks: $runHooks,
								// TODO: Use correct value!
								okWithForce: true,
								uiState
							})
						: undefined}
					{@const tooltip = commitStatusLabel(commit.state.type)}
					<Dropzone handlers={[amendHandler, squashHandler, hunkHandler].filter(isDefined)}>
						{#snippet overlay({ hovered, activated, handler })}
							{@const label =
								handler instanceof AmendCommitWithChangeDzHandler ||
								handler instanceof AmendCommitWithHunkDzHandler
									? 'Amend'
									: 'Squash'}
							<CardOverlay {hovered} {activated} {label} />
						{/snippet}
						<div
							data-remove-from-panning
							use:draggableCommitV3={{
								disabled: false,
								label: commit.message.split('\n')[0],
								sha: commit.id.slice(0, 7),
								date: getTimeAgo(commit.createdAt),
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
													isLocalAndRemoteCommit(commit) && commit.state.type === 'Integrated'
											},
											false,
											branchName
										)
									: undefined,
								viewportId: 'board-viewport',
								dropzoneRegistry,
								dragStateService
							}}
						>
							<CommitRow
								commitId={commit.id}
								commitMessage={commit.message}
								type={commit.state.type}
								hasConflicts={commit.hasConflicts}
								diverged={commit.state.type === 'LocalAndRemote' &&
									commit.id !== commit.state.subject}
								createdAt={commit.createdAt}
								{stackId}
								{branchName}
								{first}
								lastCommit={last}
								{lastBranch}
								{selected}
								{tooltip}
								{active}
								onclick={() => handleCommitClick(commit.id, false)}
								disableCommitActions={false}
							>
								{#snippet menu({ rightClickTrigger })}
									{@const data = {
										stackId,
										commitId,
										commitMessage: commit.message,
										commitStatus: commit.state.type,
										commitUrl: forge.current.commitUrl(commitId),
										onUncommitClick: () => handleUncommit(commit.id, branchName),
										onEditMessageClick: () => startEditingCommitMessage(branchName, commit.id),
										onPatchEditClick: () =>
											handleEditPatch({
												commitId: commit.id,
												type: commit.state.type,
												hasConflicts: hasConflicts(commit),
												isAncestorMostConflicted: ancestorMostConflicted?.id === commit.id
											})
									}}
									<CommitContextMenu flat {projectId} {rightClickTrigger} contextData={data} />
								{/snippet}
							</CommitRow>
						</div>
					</Dropzone>
					{@render commitReorderDz(
						stackingReorderDropzoneManager.belowCommit(branchName, commit.id)
					)}
					{#if isCommitting && last}
						<CommitGoesHere
							commitId={baseSha}
							{first}
							{last}
							selected={exclusiveAction?.type === 'commit' &&
								exclusiveAction.parentCommitId === baseSha}
							onclick={() => {
								projectState.exclusiveAction.set({
									type: 'commit',
									stackId,
									branchName,
									parentCommitId: baseSha
								});
							}}
						/>
					{/if}
				{/each}
			</div>
		{/if}
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.commit-list {
		display: flex;
		position: relative;
		flex-direction: column;
		border-top: 1px solid var(--clr-border-2);
	}
</style>
