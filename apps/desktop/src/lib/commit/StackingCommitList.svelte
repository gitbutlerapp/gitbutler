<script lang="ts">
	import CommitAction from './CommitAction.svelte';
	import CommitCard from './StackingCommitCard.svelte';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { transformAnyCommit } from '$lib/commitLines/transformers';
	import InsertEmptyCommitAction from '$lib/components/InsertEmptyCommitAction.svelte';
	import {
		ReorderDropzoneManagerFactory,
		type ReorderDropzone
	} from '$lib/dragging/reorderDropzoneManager';
	import Dropzone from '$lib/dropzone/Dropzone.svelte';
	import LineOverlay from '$lib/dropzone/LineOverlay.svelte';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import { getContext } from '$lib/utils/context';
	import { getContextStore } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { Commit, DetailedCommit, VirtualBranch } from '$lib/vbranches/types';
	import Button from '@gitbutler/ui/Button.svelte';
	import Line from '@gitbutler/ui/commitLinesStacking/Line.svelte';
	import { LineManagerFactory } from '@gitbutler/ui/commitLinesStacking/lineManager';
	import type { Snippet } from 'svelte';

	interface Props {
		localCommits: DetailedCommit[];
		localAndRemoteCommits: DetailedCommit[];
		integratedCommits: DetailedCommit[];
		remoteCommits: Commit[];
		isUnapplied: boolean;
		pushButton?: Snippet<[{ disabled: boolean }]>;
		localCommitsConflicted: boolean;
		localAndRemoteCommitsConflicted: boolean;
	}
	const {
		localCommits,
		localAndRemoteCommits,
		integratedCommits,
		remoteCommits,
		isUnapplied,
		pushButton,
		localAndRemoteCommitsConflicted
	}: Props = $props();

	const branch = getContextStore(VirtualBranch);
	const baseBranch = getContextStore(BaseBranch);
	const branchController = getContext(BranchController);
	const lineManagerFactory = getContext(LineManagerFactory);

	const reorderDropzoneManagerFactory = getContext(ReorderDropzoneManagerFactory);
	const gitHost = getGitHost();

	// TODO: Why does eslint-svelte-plugin complain about enum?
	// eslint-disable-next-line svelte/valid-compile
	enum LineSpacer {
		Remote = 'remote-spacer',
		Local = 'local-spacer',
		LocalAndRemote = 'local-and-remote-spacer'
	}

	const mappedRemoteCommits = $derived(
		remoteCommits.length > 0
			? [...remoteCommits.map(transformAnyCommit), { id: LineSpacer.Remote }]
			: []
	);

	const mappedLocalCommits = $derived(
		localCommits.length > 0 ? localCommits.map(transformAnyCommit) : []
	);
	const mappedLocalAndRemoteCommits = $derived(
		localAndRemoteCommits.length > 0
			? [...localAndRemoteCommits.map(transformAnyCommit), { id: LineSpacer.LocalAndRemote }]
			: []
	);

	const forkPoint = $derived($branch.forkPoint);
	const upstreamForkPoint = $derived($branch.upstreamData?.forkPoint);
	const isRebased = $derived(!!forkPoint && !!upstreamForkPoint && forkPoint !== upstreamForkPoint);

	const lineManager = $derived(
		lineManagerFactory.build(
			{
				remoteCommits: mappedRemoteCommits,
				localCommits: mappedLocalCommits,
				localAndRemoteCommits: mappedLocalAndRemoteCommits,
				integratedCommits: integratedCommits.map(transformAnyCommit)
			},
			!isRebased
		)
	);

	const hasCommits = $derived($branch.commits && $branch.commits.length > 0);
	const headCommit = $derived($branch.commits.at(0));

	const hasRemoteCommits = $derived(remoteCommits.length > 0);

	const reorderDropzoneManager = $derived(
		reorderDropzoneManagerFactory.build($branch, [...localCommits, ...localAndRemoteCommits])
	);

	let isIntegratingCommits = $state(false);

	function insertBlankCommit(commitId: string, location: 'above' | 'below' = 'below') {
		if (!$branch || !$baseBranch) {
			console.error('Unable to insert commit');
			return;
		}
		branchController.insertBlankCommit($branch.id, commitId, location === 'above' ? -1 : 1);
	}

	function getReorderDropzoneOffset({
		isFirst = false,
		isMiddle = false,
		isLast = false
	}: {
		isFirst?: boolean;
		isMiddle?: boolean;
		isLast?: boolean;
	}) {
		if (isFirst) return 12;
		if (isMiddle) return 6;
		if (isLast) return 0;
		return 0;
	}
</script>

{#snippet reorderDropzone(dropzone: ReorderDropzone, yOffsetPx: number)}
	<Dropzone accepts={dropzone.accepts.bind(dropzone)} ondrop={dropzone.onDrop.bind(dropzone)}>
		{#snippet overlay({ hovered, activated })}
			<LineOverlay {hovered} {activated} {yOffsetPx} />
		{/snippet}
	</Dropzone>
{/snippet}

{#if hasCommits || hasRemoteCommits}
	<div class="commits stacked">
		<!-- UPSTREAM COMMITS -->

		{#if remoteCommits.length > 0}
			<!-- To make the sticky position work, commits should be wrapped in a div -->
			<div class="commits-group">
				{#each remoteCommits as commit, idx (commit.id)}
					<CommitCard
						type="remote"
						branch={$branch}
						{commit}
						{isUnapplied}
						first={idx === 0}
						last={idx === remoteCommits.length - 1}
						commitUrl={$gitHost?.commitUrl(commit.id)}
						isHeadCommit={commit.id === headCommit?.id}
					>
						{#snippet lines(topHeightPx)}
							<Line line={lineManager.get(commit.id)} {topHeightPx} />
						{/snippet}
					</CommitCard>
				{/each}

				<CommitAction>
					{#snippet lines()}
						<Line line={lineManager.get(LineSpacer.Remote)} topHeightPx={0} />
					{/snippet}
					{#snippet action()}
						<Button
							style="warning"
							kind="solid"
							loading={isIntegratingCommits}
							onclick={async () => {
								isIntegratingCommits = true;
								try {
									await branchController.mergeUpstream($branch.id);
								} catch (e) {
									console.error(e);
								} finally {
									isIntegratingCommits = false;
								}
							}}>Integrate upstream</Button
						>
					{/snippet}
				</CommitAction>
			</div>
		{/if}

		<!-- LOCAL COMMITS -->
		{#if localCommits.length > 0}
			<div class="commits-group">
				<InsertEmptyCommitAction
					isFirst
					on:click={() => insertBlankCommit($branch.head, 'above')}
				/>
				{@render reorderDropzone(
					reorderDropzoneManager.topDropzone,
					getReorderDropzoneOffset({ isFirst: true })
				)}
				{#each localCommits as commit, idx (commit.id)}
					<CommitCard
						{commit}
						{isUnapplied}
						type="local"
						first={idx === 0}
						branch={$branch}
						last={idx === localCommits.length - 1}
						isHeadCommit={commit.id === headCommit?.id}
					>
						{#snippet lines(topHeightPx)}
							<Line line={lineManager.get(commit.id)} {topHeightPx} />
						{/snippet}
					</CommitCard>

					{@render reorderDropzone(
						reorderDropzoneManager.dropzoneBelowCommit(commit.id),
						getReorderDropzoneOffset({
							isLast: idx + 1 === localCommits.length,
							isMiddle: idx + 1 === localCommits.length
						})
					)}

					<InsertEmptyCommitAction
						isLast={idx + 1 === localCommits.length}
						on:click={() => insertBlankCommit(commit.id, 'below')}
					/>
				{/each}
			</div>
		{/if}

		<!-- LOCAL AND REMOTE COMMITS -->
		{#if localAndRemoteCommits.length > 0}
			<div class="commits-group">
				{#each localAndRemoteCommits as commit, idx (commit.id)}
					<CommitCard
						{commit}
						{isUnapplied}
						type="localAndRemote"
						first={idx === 0}
						branch={$branch}
						last={idx === localAndRemoteCommits.length - 1}
						isHeadCommit={commit.id === headCommit?.id}
						commitUrl={$gitHost?.commitUrl(commit.id)}
					>
						{#snippet lines(topHeightPx)}
							<Line line={lineManager.get(commit.id)} {topHeightPx} />
						{/snippet}
					</CommitCard>
					{@render reorderDropzone(
						reorderDropzoneManager.dropzoneBelowCommit(commit.id),
						getReorderDropzoneOffset({
							isMiddle: idx + 1 === localAndRemoteCommits.length
						})
					)}
					<InsertEmptyCommitAction
						isLast={idx + 1 === localAndRemoteCommits.length}
						on:click={() => insertBlankCommit(commit.id, 'below')}
					/>
				{/each}

				{#if remoteCommits.length > 0 && localCommits.length === 0 && pushButton}
					<CommitAction>
						{#snippet lines()}
							<Line line={lineManager.get(LineSpacer.LocalAndRemote)} topHeightPx={0} />
						{/snippet}
						{#snippet action()}
							{@render pushButton({ disabled: localAndRemoteCommitsConflicted })}
						{/snippet}
					</CommitAction>
				{/if}
			</div>
		{/if}

		<!-- INTEGRATED COMMITS -->
		{#if integratedCommits.length > 0}
			<div class="commits-group">
				{#each integratedCommits as commit, idx (commit.id)}
					<CommitCard
						{commit}
						{isUnapplied}
						type="integrated"
						first={idx === 0}
						branch={$branch}
						isHeadCommit={commit.id === headCommit?.id}
						last={idx === integratedCommits.length - 1}
						commitUrl={$gitHost?.commitUrl(commit.id)}
					>
						{#snippet lines(topHeightPx)}
							<Line line={lineManager.get(commit.id)} {topHeightPx} />
						{/snippet}
					</CommitCard>
				{/each}
			</div>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.commits {
		position: relative;
		display: flex;
		flex-direction: column;
		background-color: var(--clr-bg-2);
		border-top: 1px solid var(--clr-border-2);
		border-bottom-left-radius: var(--radius-m);
		border-bottom-right-radius: var(--radius-m);

		--base-top-margin: 8px;
		--base-icon-top: -8px;

		--avatar-first-top: 50px;
		--avatar-top: 16px;
	}

	.commits.stacked {
		border-top: none;
	}
</style>
