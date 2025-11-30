<script lang="ts">
	import BranchIntegrationModal from '$components/BranchIntegrationModal.svelte';
	import CardOverlay from '$components/CardOverlay.svelte';
	import CommitContextMenu from '$components/CommitContextMenu.svelte';
	import CommitGoesHere from '$components/CommitGoesHere.svelte';
	import CommitLineOverlay from '$components/CommitLineOverlay.svelte';
	import CommitRow from '$components/CommitRow.svelte';
	import Dropzone from '$components/Dropzone.svelte';

	import ReduxResult from '$components/ReduxResult.svelte';
	import UpstreamCommitsAction from '$components/UpstreamCommitsAction.svelte';
	import { isLocalAndRemoteCommit, isUpstreamCommit } from '$components/lib';
	import { commitCreatedAt } from '$lib/branches/v3';
	import { commitStatusLabel } from '$lib/commits/commit';
	import {
		AmendCommitWithChangeDzHandler,
		AmendCommitWithHunkDzHandler,
		CommitDropData,
		type DzCommitData,
		SquashCommitDzHandler
	} from '$lib/commits/dropHandler';
	import { projectRunCommitHooks } from '$lib/config/config';
	import { draggableCommitV3 } from '$lib/dragging/draggable';
	import { DROPZONE_REGISTRY } from '$lib/dragging/registry';
	import {
		ReorderCommitDzFactory,
		ReorderCommitDzHandler
	} from '$lib/dragging/stackingReorderDropzoneManager';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { HOOKS_SERVICE } from '$lib/hooks/hooksService';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/core/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import { Button, Modal, RadioButton, TestId } from '@gitbutler/ui';
	import { DRAG_STATE_SERVICE } from '@gitbutler/ui/drag/dragStateService.svelte';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import type { BranchDetails } from '$lib/stacks/stack';

	interface Props {
		projectId: string;
		stackId?: string;
		laneId: string;
		branchName: string;
		lastBranch: boolean;
		branchDetails: BranchDetails;
		stackingReorderDropzoneManager: ReorderCommitDzFactory;
		roundedTop?: boolean;
		active?: boolean;

		handleUncommit: (commitId: string, branchName: string) => Promise<void>;
		startEditingCommitMessage: (branchName: string, commitId: string) => void;
		onselect?: () => void;
	}

	let {
		projectId,
		stackId,
		laneId,
		branchName,
		branchDetails,
		lastBranch,
		stackingReorderDropzoneManager,
		roundedTop,
		active,
		handleUncommit,
		startEditingCommitMessage,
		onselect
	}: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const uiState = inject(UI_STATE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const hooksService = inject(HOOKS_SERVICE);
	const dropzoneRegistry = inject(DROPZONE_REGISTRY);
	const dragStateService = inject(DRAG_STATE_SERVICE);

	const [integrateUpstreamCommits, integrating] = stackService.integrateUpstreamCommits;

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

	let integrationModal = $state<Modal>();

	async function handleCommitClick(commitId: string, upstream: boolean) {
		const currentSelection = laneState.selection.current;
		// Toggle: if this exact commit is already selected, clear the selection
		if (currentSelection?.commitId === commitId && currentSelection?.branchName === branchName) {
			laneState.selection.set(undefined);
		} else {
			laneState.selection.set({ branchName, commitId, upstream, previewOpen: true });
		}
		onselect?.();
	}

	function kickOffIntegration() {
		integrationModal?.show();
	}

	function handleRebaseIntegration() {
		if (!stackId) return;
		integrateUpstreamCommits({
			projectId,
			stackId,
			seriesName: branchName,
			strategy: 'rebase'
		});
	}

	type IntegrationMode = 'rebase' | 'interactive';

	const integrationMode = persisted<IntegrationMode>('rebase', 'branchUpstreamIntegrationMode');

	function integrate(mode: IntegrationMode) {
		switch (mode) {
			case 'rebase':
				handleRebaseIntegration();
				break;
			case 'interactive':
				kickOffIntegration();
				break;
		}
	}

	function getLabelForIntegrationMode(mode: IntegrationMode): string {
		switch (mode) {
			case 'rebase':
				return 'Rebase';
			case 'interactive':
				return 'Configure integration…';
		}
	}
</script>

<BranchIntegrationModal bind:modalRef={integrationModal} {projectId} {stackId} {branchName} />

{#snippet integrateUpstreamAction()}
	<form
		class="uppstream-integration-actions"
		onsubmit={() => {
			integrate($integrationMode);
		}}
	>
		<div class="uppstream-integration-actions__radio-container">
			{@render integrationRadioOption(
				'rebase',
				'Rebase upstream changes',
				'Place local-only changes on top, then the upstream changes. Similar to git pull --rebase.'
			)}
			{@render integrationRadioOption(
				'interactive',
				'Interactive integration',
				'Review and resolve any conflicts before completing the integration.'
			)}
		</div>

		<Button
			type="submit"
			style="warning"
			disabled={integrating.current.isLoading}
			testId={TestId.UpstreamCommitsIntegrateButton}
		>
			{getLabelForIntegrationMode($integrationMode)}
		</Button>
	</form>
{/snippet}

<!-- Integration radio option snippet -->
{#snippet integrationRadioOption(mode: IntegrationMode, title: string, description: string)}
	<label class="integration-radio-option" class:selected={$integrationMode === mode}>
		<div class="integration-radio-content">
			<h4 class="text-13 text-semibold">{title}</h4>
			<p class="text-11 text-body clr-text-2">
				{description}
			</p>
		</div>
		<RadioButton
			class="integration-radio-option__radio"
			name="integrationMode"
			value={mode}
			checked={$integrationMode === mode}
			onchange={() => integrationMode.set(mode)}
		/>
	</label>
{/snippet}

{#snippet commitReorderDz(dropzone: ReorderCommitDzHandler)}
	{#if !isCommitting}
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
						{#if !isCommitting}
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
								{active}
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
							{@render integrateUpstreamAction()}
						{/snippet}
					</UpstreamCommitsAction>
				{/if}

				{@render commitReorderDz(stackingReorderDropzoneManager.top(branchName))}

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
								(newId) => {
									const previewOpen = selection.current?.previewOpen ?? false;
									uiState.lane(stackId).selection.set({ branchName, commitId: newId, previewOpen });
								},
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
								createdAt={commitCreatedAt(commit)}
								gerritReviewUrl={commit.gerritReviewUrl ?? undefined}
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
								editable={!!stackId}
							>
								{#snippet menu({ rightClickTrigger })}
									{@const data = {
										stackId,
										commitId,
										commitMessage: commit.message,
										commitStatus: commit.state.type,
										commitUrl: forge.current.commitUrl(commitId),
										onUncommitClick: () => handleUncommit(commit.id, branchName),
										onEditMessageClick: () => startEditingCommitMessage(branchName, commit.id)
									}}
									<CommitContextMenu
										showOnHover
										{projectId}
										{rightClickTrigger}
										contextData={data}
									/>
								{/snippet}
							</CommitRow>
						</div>
					</Dropzone>
					{@render commitReorderDz(
						stackingReorderDropzoneManager.belowCommit(branchName, commit.id)
					)}
					{#if isCommitting && last}
						<CommitGoesHere
							commitId={branchDetails.baseCommit}
							{first}
							{last}
							selected={exclusiveAction?.type === 'commit' &&
								exclusiveAction.parentCommitId === branchDetails.baseCommit &&
								commitAction?.branchName === branchName}
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
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: 0 0 var(--radius-ml) var(--radius-ml);
		background-color: var(--clr-bg-1);

		&.rounded {
			border-radius: var(--radius-ml);
		}
	}

	.uppstream-integration-actions {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	.uppstream-integration-actions__radio-container {
		display: flex;
		flex-direction: column;
	}

	.integration-radio-option {
		display: flex;
		z-index: 0;
		position: relative;
		padding: 14px;
		gap: 20px;
		border: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
		cursor: pointer;

		&:not(.selected):hover {
			background: var(--clr-bg-1-muted);
		}

		&:first-child {
			border-top-right-radius: var(--radius-m);
			border-top-left-radius: var(--radius-m);
		}
		&:last-child {
			margin-top: -1px;
			border-bottom-right-radius: var(--radius-m);
			border-bottom-left-radius: var(--radius-m);
		}

		&.selected {
			z-index: 1;
			border-color: var(--clr-theme-pop-element);
			background: var(--clr-theme-pop-bg);
		}
	}

	:global(.integration-radio-option__radio) {
		flex-shrink: 0;
	}

	.integration-radio-content {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: 6px;
	}
</style>
