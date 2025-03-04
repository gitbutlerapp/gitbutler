<script lang="ts">
	import CommitAction from './CommitAction.svelte';
	import CommitCard from './CommitCard.svelte';
	import CommitDragItem from './CommitDragItem.svelte';
	import CommitsAccordion from './CommitsAccordion.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import LineOverlay from '$components/LineOverlay.svelte';
	import { BranchStack } from '$lib/branches/branch';
	import { PatchSeries } from '$lib/branches/branch';
	import { BranchController, type SeriesIntegrationStrategy } from '$lib/branches/branchController';
	import { findLastDivergentCommit } from '$lib/commits/utils';
	import {
		StackingReorderDropzoneManager,
		type StackingReorderDropzone
	} from '$lib/dragging/stackingReorderDropzoneManager';
	import { getForge } from '$lib/forge/interface/forge';
	import { getContext } from '@gitbutler/shared/context';
	import { getContextStore } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Line from '@gitbutler/ui/commitLines/Line.svelte';
	import { LineManagerFactory } from '@gitbutler/ui/commitLines/lineManager';

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
			style: 'ghost',
			kind: 'outline',
			icon: 'warning-small',
			action: confirmReset
		}
	} as const;

	type IntegrationStrategy = keyof typeof integrationStrategies;

	interface Props {
		currentSeries: PatchSeries;
		isUnapplied: boolean;
		stackingReorderDropzoneManager: StackingReorderDropzoneManager;
		isBottom?: boolean;
	}
	const {
		currentSeries,
		isUnapplied,
		stackingReorderDropzoneManager,
		isBottom = false
	}: Props = $props();

	const stack = getContextStore(BranchStack);
	const branchController = getContext(BranchController);
	const lineManagerFactory = getContext(LineManagerFactory);

	const forge = getForge();

	const localAndRemoteCommits = $derived(
		currentSeries.patches.filter((patch) => patch.status === 'localAndRemote')
	);
	const lastDivergentCommit = $derived(
		findLastDivergentCommit(currentSeries.upstreamPatches, localAndRemoteCommits)
	);

	const remoteOnlyPatches = $derived(
		currentSeries.upstreamPatches.filter((patch) => patch.status !== 'integrated')
	);
	const remoteIntegratedPatches = $derived(
		currentSeries.upstreamPatches.filter((patch) => patch.status === 'integrated')
	);

	// A local or localAndRemote commit probably shouldn't every be integrated,
	// but the isIntegrated check is a bit fuzzy, and is certainly the most
	// important state to convey to the user.
	const lineManager = $derived(
		lineManagerFactory.build({
			remoteCommits: remoteOnlyPatches,
			localCommits: currentSeries.patches.filter((patch) => patch.status === 'local'),
			localAndRemoteCommits: localAndRemoteCommits,
			integratedCommits: [
				...currentSeries.patches.filter((patch) => patch.status === 'integrated'),
				...remoteIntegratedPatches
			]
		})
	);

	const hasCommits = $derived(currentSeries.patches.length > 0);
	const headCommit = $derived(currentSeries.patches.at(0));

	const hasRemoteCommits = $derived(remoteOnlyPatches.length > 0);
	const hasRemoteIntegratedCommits = $derived(remoteIntegratedPatches.length > 0);
	let isIntegratingCommits = $state(false);

	let confirmResetModal = $state<ReturnType<typeof Modal>>();

	async function integrate(strategy?: SeriesIntegrationStrategy): Promise<void> {
		isIntegratingCommits = true;
		try {
			await branchController.integrateUpstreamForSeries($stack.id, currentSeries.name, strategy);
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
	{@const { label, icon, style, kind, action } = integrationStrategies[strategy]}
	<Button
		{style}
		{kind}
		grow
		{icon}
		reversedDirection
		loading={isIntegratingCommits}
		onclick={action}
	>
		{label}
	</Button>
{/snippet}

{#if hasCommits || hasRemoteCommits}
	<div class="commits">
		<!-- UPSTREAM ONLY COMMITS -->
		{#if hasRemoteCommits}
			<CommitsAccordion
				count={Math.min(currentSeries.upstreamPatches.length, 3)}
				isLast={!hasCommits}
				type="upstream"
				displayHeader={currentSeries.upstreamPatches.length > 1}
			>
				{#snippet title()}
					<span class="text-13 text-body text-semibold">Upstream commits</span>
				{/snippet}
				{#each remoteOnlyPatches as commit, idx (commit.id)}
					<CommitCard
						type="remote"
						branch={$stack}
						{commit}
						{isUnapplied}
						{currentSeries}
						noBorder={idx === currentSeries.upstreamPatches.length - 1}
						commitUrl={$forge?.commitUrl(commit.id)}
						isHeadCommit={commit.id === headCommit?.id}
					>
						{#snippet lines()}
							<Line
								line={lineManager.get(commit.id)}
								isBottom={!hasCommits && idx === currentSeries.upstreamPatches.length - 1}
							/>
						{/snippet}
					</CommitCard>
				{/each}

				<CommitAction type="remote" isLast={!hasCommits}>
					{#snippet action()}
						{@render integrateUpstreamButton('default')}
					{/snippet}
				</CommitAction>
			</CommitsAccordion>
		{/if}

		<!-- REMAINING LOCAL, LOCALANDREMOTE, AND INTEGRATED COMMITS -->
		{#if currentSeries.patches.length > 0}
			<div class="commits-group">
				{@render stackingReorderDropzone(
					stackingReorderDropzoneManager.topDropzone(currentSeries.name)
				)}

				{#each currentSeries.patches as commit, idx (commit.id)}
					{@const isResetAction =
						!hasRemoteIntegratedCommits &&
						((lastDivergentCommit.type === 'localDiverged' &&
							lastDivergentCommit.commit.id === commit.id) ||
							(lastDivergentCommit.type === 'onlyRemoteDiverged' &&
								idx === currentSeries.patches.length - 1))}
					<CommitDragItem {commit}>
						<CommitCard
							type={commit.status}
							branch={$stack}
							{commit}
							{isUnapplied}
							{currentSeries}
							noBorder={idx === currentSeries.patches.length - 1}
							last={idx === currentSeries.patches.length - 1 && !isResetAction}
							isHeadCommit={commit.id === headCommit?.id}
							commitUrl={$forge?.commitUrl(commit.id)}
						>
							{#snippet lines()}
								<Line
									line={lineManager.get(commit.id)}
									isBottom={isBottom && idx === currentSeries.patches.length - 1}
								/>
							{/snippet}
						</CommitCard>
					</CommitDragItem>

					{@render stackingReorderDropzone(
						stackingReorderDropzoneManager.dropzoneBelowCommit(currentSeries.name, commit.id)
					)}

					<!-- RESET TO REMOTE BUTTON -->
					{#if isResetAction}
						<CommitAction type="local" isLast={idx === currentSeries.patches.length - 1}>
							{#snippet action()}
								{@render integrateUpstreamButton('reset')}
							{/snippet}
						</CommitAction>
					{/if}
				{/each}
			</div>
		{/if}

		<!-- REMOTE INTEGRATED COMMITS -->
		{#if hasRemoteIntegratedCommits}
			<CommitsAccordion
				count={Math.min(remoteIntegratedPatches.length, 3)}
				isLast={!hasCommits}
				type="integrated"
				alignTop
				displayHeader
				unfoldable={remoteIntegratedPatches.length <= 1}
			>
				{#snippet title()}
					<span class="text-12 text-body"
						>Some branches in this stack have been integrated. Please force push to sync your branch
						with the updated base ↘</span
					>
				{/snippet}
				{#each remoteIntegratedPatches as commit, idx (commit.id)}
					<CommitCard
						type={commit.status}
						branch={$stack}
						{commit}
						{currentSeries}
						{isUnapplied}
						noBorder={idx === remoteIntegratedPatches.length - 1}
						last={idx === remoteIntegratedPatches.length - 1}
						isHeadCommit={commit.id === headCommit?.id}
						commitUrl={$forge?.commitUrl(commit.id)}
						disableCommitActions={true}
					>
						{#snippet lines()}
							<Line
								line={lineManager.get(commit.id)}
								isBottom={isBottom && idx === currentSeries.patches.length - 1}
							/>
						{/snippet}
					</CommitCard>
				{/each}
			</CommitsAccordion>
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
			<p class="text-13 text-body helper-text">
				This will reset the branch to the state of the remote branch. All local changes will be
				overwritten.
			</p>
		{/snippet}
		{#snippet controls(close)}
			<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
			<Button style="error" type="submit">Reset</Button>
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
