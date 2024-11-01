<script lang="ts">
	import CommitAction from './CommitAction.svelte';
	import StackingCommitCard from './StackingCommitCard.svelte';
	import StackingCommitDragItem from './StackingCommitDragItem.svelte';
	import StackingUpstreamCommitsAccordion from './StackingUpstreamCommitsAccordion.svelte';
	import {
		StackingReorderDropzoneManager,
		type StackingReorderDropzone
	} from '$lib/dragging/stackingReorderDropzoneManager';
	import Dropzone from '$lib/dropzone/Dropzone.svelte';
	import LineOverlay from '$lib/dropzone/LineOverlay.svelte';
	import { getForge } from '$lib/forge/interface/forge';
	import { BranchController } from '$lib/vbranches/branchController';
	import { DetailedCommit, VirtualBranch, type CommitStatus } from '$lib/vbranches/types';
	import { getContext } from '@gitbutler/shared/context';
	import { getContextStore } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Line from '@gitbutler/ui/commitLinesStacking/Line.svelte';
	import { LineManagerFactory } from '@gitbutler/ui/commitLinesStacking/lineManager';
	import type { Snippet } from 'svelte';

	interface Props {
		remoteOnlyPatches: DetailedCommit[];
		patches: DetailedCommit[];
		seriesName: string;
		isUnapplied: boolean;
		pushButton?: Snippet<[{ disabled: boolean }]>;
		hasConflicts: boolean;
		stackingReorderDropzoneManager: StackingReorderDropzoneManager;
		isBottom?: boolean;
	}
	const {
		remoteOnlyPatches,
		patches,
		seriesName,
		isUnapplied,
		pushButton,
		hasConflicts,
		stackingReorderDropzoneManager,
		isBottom = false
	}: Props = $props();

	const branch = getContextStore(VirtualBranch);
	const branchController = getContext(BranchController);
	const lineManagerFactory = getContext(LineManagerFactory);

	const forge = getForge();

	const LineSpacer = {
		Remote: 'remote-spacer',
		Local: 'local-spacer',
		LocalAndRemote: 'local-and-remote-spacer'
	};

	// TODO: Refactor out lineManager; unnecessary in stacking context
	const lineManager = $derived(
		lineManagerFactory.build({
			remoteCommits: remoteOnlyPatches,
			localCommits: patches.filter((patch) => !patch.remoteCommitId),
			localAndRemoteCommits: patches.filter((patch) => patch.remoteCommitId),
			integratedCommits: patches.filter((patch) => patch.isIntegrated)
		})
	);

	const hasCommits = $derived($branch.commits && $branch.commits.length > 0);
	const headCommit = $derived($branch.commits.at(0));

	const hasRemoteCommits = $derived(remoteOnlyPatches.length > 0);

	let isIntegratingCommits = $state(false);

	const topPatch = $derived(patches[0]);
	const branchType = $derived<CommitStatus>(topPatch?.status ?? 'local');
	const isBranchIntegrated = $derived(branchType === 'integrated');
</script>

{#snippet stackingReorderDropzone(dropzone: StackingReorderDropzone)}
	<Dropzone accepts={dropzone.accepts.bind(dropzone)} ondrop={dropzone.onDrop.bind(dropzone)}>
		{#snippet overlay({ hovered, activated })}
			<LineOverlay {hovered} {activated} />
		{/snippet}
	</Dropzone>
{/snippet}

{#if hasCommits || hasRemoteCommits}
	<div class="commits">
		<!-- UPSTREAM ONLY COMMITS -->
		{#if hasRemoteCommits}
			<StackingUpstreamCommitsAccordion count={Math.min(remoteOnlyPatches.length, 3)}>
				{#each remoteOnlyPatches as commit, idx (commit.id)}
					<StackingCommitCard
						type="remote"
						branch={$branch}
						{commit}
						{isUnapplied}
						last={idx === remoteOnlyPatches.length - 1}
						commitUrl={$forge?.commitUrl(commit.id)}
						isHeadCommit={commit.id === headCommit?.id}
					>
						{#snippet lines()}
							<Line line={lineManager.get(commit.id)} />
						{/snippet}
					</StackingCommitCard>
				{/each}
				{#snippet action()}
					<Button
						style="warning"
						kind="solid"
						grow
						loading={isIntegratingCommits}
						onclick={async () => {
							isIntegratingCommits = true;
							try {
								await branchController.mergeUpstreamForSeries($branch.id, seriesName);
							} catch (e) {
								console.error(e);
							} finally {
								isIntegratingCommits = false;
							}
						}}
					>
						Integrate upstream
					</Button>
				{/snippet}
			</StackingUpstreamCommitsAccordion>
		{/if}

		<!-- REMAINING LOCAL, LOCALANDREMOTE, AND INTEGRATED COMMITS -->
		{#if patches.length > 0}
			<div class="commits-group">
				{@render stackingReorderDropzone(stackingReorderDropzoneManager.topDropzone(seriesName))}

				{#each patches as commit, idx (commit.id)}
					<StackingCommitDragItem {commit}>
						<StackingCommitCard
							type={commit.status}
							branch={$branch}
							{commit}
							{seriesName}
							{isUnapplied}
							last={idx === patches.length - 1}
							isHeadCommit={commit.id === headCommit?.id}
							commitUrl={$forge?.commitUrl(commit.id)}
						>
							{#snippet lines()}
								<Line
									line={lineManager.get(commit.id)}
									isBottom={isBottom && idx === patches.length - 1}
								/>
							{/snippet}
						</StackingCommitCard>
					</StackingCommitDragItem>

					{@render stackingReorderDropzone(
						stackingReorderDropzoneManager.dropzoneBelowCommit(seriesName, commit.id)
					)}
				{/each}
			</div>
		{/if}
		{#if remoteOnlyPatches.length > 0 && patches.length === 0 && !isBranchIntegrated && pushButton}
			<CommitAction>
				{#snippet lines()}
					<Line line={lineManager.get(LineSpacer.LocalAndRemote)} />
				{/snippet}
				{#snippet action()}
					{@render pushButton({ disabled: hasConflicts })}
				{/snippet}
			</CommitAction>
		{/if}
	</div>
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
