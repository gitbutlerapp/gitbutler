<script lang="ts">
	import CommitAction from './CommitAction.svelte';
	import CommitCard from './CommitCard.svelte';
	import CommitDragItem from './CommitDragItem.svelte';
	import UpstreamCommitsAccordion from './UpstreamCommitsAccordion.svelte';
	import { findLastDivergentCommit } from '$lib/commits/utils';
	import {
		StackingReorderDropzoneManager,
		type StackingReorderDropzone
	} from '$lib/dragging/stackingReorderDropzoneManager';
	import Dropzone from '$lib/dropzone/Dropzone.svelte';
	import LineOverlay from '$lib/dropzone/LineOverlay.svelte';
	import { getForge } from '$lib/forge/interface/forge';
	import {
		BranchController,
		type SeriesIntegrationStrategy
	} from '$lib/vbranches/branchController';
	import { DetailedCommit, VirtualBranch } from '$lib/vbranches/types';
	import { getContext } from '@gitbutler/shared/context';
	import { getContextStore } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Line from '@gitbutler/ui/commitLines/Line.svelte';
	import { LineManagerFactory } from '@gitbutler/ui/commitLines/lineManager';

	const integrationStrategies = {
		default: {
			label: 'Integrate upstream',
			stretegy: undefined,
			style: 'warning',
			action: integrate
		},
		reset: {
			label: 'Reset to remote…',
			stretegy: 'hardreset',
			style: 'error',
			action: confirmReset
		}
	} as const;

	type IntegrationStrategy = keyof typeof integrationStrategies;

	interface Props {
		remoteOnlyPatches: DetailedCommit[];
		patches: DetailedCommit[];
		seriesName: string;
		isUnapplied: boolean;
		stackingReorderDropzoneManager: StackingReorderDropzoneManager;
		isBottom?: boolean;
	}
	const {
		remoteOnlyPatches,
		patches,
		seriesName,
		isUnapplied,
		stackingReorderDropzoneManager,
		isBottom = false
	}: Props = $props();

	const branch = getContextStore(VirtualBranch);
	const branchController = getContext(BranchController);
	const lineManagerFactory = getContext(LineManagerFactory);

	const forge = getForge();

	const localAndRemoteCommits = $derived(patches.filter((patch) => patch.remoteCommitId));
	const lastDivergentCommit = $derived(findLastDivergentCommit(localAndRemoteCommits));

	const lineManager = $derived(
		lineManagerFactory.build({
			remoteCommits: remoteOnlyPatches,
			localCommits: patches.filter((patch) => !patch.remoteCommitId),
			localAndRemoteCommits,
			integratedCommits: patches.filter((patch) => patch.isIntegrated)
		})
	);

	const hasCommits = $derived($branch.commits && $branch.commits.length > 0);
	const headCommit = $derived($branch.commits.at(0));

	const hasRemoteCommits = $derived(remoteOnlyPatches.length > 0);
	let isIntegratingCommits = $state(false);

	// const topPatch = $derived(patches[0]);
	// const branchType = $derived<CommitStatus>(topPatch?.status ?? 'local');
	// const isBranchIntegrated = $derived(branchType === 'integrated');

	let confirmResetModal = $state<ReturnType<typeof Modal>>();

	async function integrate(strategy?: SeriesIntegrationStrategy): Promise<void> {
		isIntegratingCommits = true;
		try {
			await branchController.integrateUpstreamForSeries($branch.id, seriesName, strategy);
		} catch (e) {
			console.error(e);
		} finally {
			isIntegratingCommits = false;
		}
	}

	function confirmReset() {
		confirmResetModal?.show();
	}
</script>

{#snippet stackingReorderDropzone(dropzone: StackingReorderDropzone)}
	<Dropzone accepts={dropzone.accepts.bind(dropzone)} ondrop={dropzone.onDrop.bind(dropzone)}>
		{#snippet overlay({ hovered, activated })}
			<LineOverlay {hovered} {activated} />
		{/snippet}
	</Dropzone>
{/snippet}

{#snippet integrateUpstreamButton(strategy: IntegrationStrategy)}
	{@const { label, style, action } = integrationStrategies[strategy]}
	<Button {style} kind="solid" grow loading={isIntegratingCommits} onclick={action}>
		{label}
	</Button>
{/snippet}

{#if hasCommits || hasRemoteCommits}
	<div class="commits">
		<!-- UPSTREAM ONLY COMMITS -->
		{#if hasRemoteCommits}
			<UpstreamCommitsAccordion count={Math.min(remoteOnlyPatches.length, 3)} isLast={!hasCommits}>
				{#each remoteOnlyPatches as commit, idx (commit.id)}
					<CommitCard
						type="remote"
						branch={$branch}
						{commit}
						{isUnapplied}
						noBorder={idx === remoteOnlyPatches.length - 1}
						commitUrl={$forge?.commitUrl(commit.id)}
						isHeadCommit={commit.id === headCommit?.id}
					>
						{#snippet lines()}
							<Line
								line={lineManager.get(commit.id)}
								isBottom={!hasCommits && idx === remoteOnlyPatches.length - 1}
							/>
						{/snippet}
					</CommitCard>
				{/each}

				<CommitAction type="remote" isLast={!hasCommits}>
					{#snippet action()}
						{@render integrateUpstreamButton('default')}
					{/snippet}
				</CommitAction>
			</UpstreamCommitsAccordion>
		{/if}

		<!-- REMAINING LOCAL, LOCALANDREMOTE, AND INTEGRATED COMMITS -->
		{#if patches.length > 0}
			<div class="commits-group">
				{@render stackingReorderDropzone(stackingReorderDropzoneManager.topDropzone(seriesName))}

				{#each patches as commit, idx (commit.id)}
					{@const isResetAction = lastDivergentCommit?.id === commit.id}
					<CommitDragItem {commit}>
						<CommitCard
							type={commit.status}
							branch={$branch}
							{commit}
							{seriesName}
							{isUnapplied}
							noBorder={idx === patches.length - 1}
							last={idx === patches.length - 1 && !isResetAction}
							isHeadCommit={commit.id === headCommit?.id}
							commitUrl={$forge?.commitUrl(commit.id)}
						>
							{#snippet lines()}
								<Line
									line={lineManager.get(commit.id)}
									isBottom={isBottom && idx === patches.length - 1}
								/>
							{/snippet}
						</CommitCard>
					</CommitDragItem>

					{@render stackingReorderDropzone(
						stackingReorderDropzoneManager.dropzoneBelowCommit(seriesName, commit.id)
					)}

					<!-- RESET TO REMOTE BUTTON -->
					{#if isResetAction}
						<CommitAction type="local" isLast={idx === patches.length - 1}>
							{#snippet action()}
								{@render integrateUpstreamButton('reset')}
							{/snippet}
						</CommitAction>
					{/if}
				{/each}
			</div>
		{/if}
	</div>

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
		{#snippet children()}
			<p class="text-12 text-body helper-text">
				This will reset the branch to the state of the remote branch. All local changes will be
				overwritten.
			</p>
		{/snippet}
		{#snippet controls(close)}
			<Button style="ghost" outline type="reset" onclick={close}>Cancel</Button>
			<Button style="error" type="submit" kind="solid">Reset</Button>
		{/snippet}
	</Modal>
{/if}

<style lang="postcss">
	.commits {
		position: relative;
		display: flex;
		flex-direction: column;
		background-color: var(--clr-bg-2);
		border-radius: 0 0 var(--radius-m) var(--radius-m);
	}

	.commits-group {
		border-bottom: 1px solid var(--clr-border-2);

		&:last-child {
			border-bottom: none;
		}
	}
</style>
