<script lang="ts">
	import CommitAction from './CommitAction.svelte';
	import StackingCommitCard from './StackingCommitCard.svelte';
	import StackingCommitDragItem from './StackingCommitDragItem.svelte';
	import StackingUpstreamCommitsAccordion from './StackingUpstreamCommitsAccordion.svelte';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { transformAnyCommit } from '$lib/commitLines/transformers';
	import InsertEmptyCommitAction from '$lib/components/InsertEmptyCommitAction.svelte';
	import {
		ReorderDropzoneManager,
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
		reorderDropzoneManager: ReorderDropzoneManager;
	}
	const {
		localCommits,
		localAndRemoteCommits,
		integratedCommits,
		remoteCommits,
		isUnapplied,
		pushButton,
		localAndRemoteCommitsConflicted,
		reorderDropzoneManager
	}: Props = $props();

	const branch = getContextStore(VirtualBranch);
	const baseBranch = getContextStore(BaseBranch);
	const branchController = getContext(BranchController);
	const lineManagerFactory = getContext(LineManagerFactory);

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
		localCommits.length > 0
			? [...localCommits.map(transformAnyCommit), { id: LineSpacer.Local }]
			: []
	);
	const mappedLocalAndRemoteCommits = $derived(
		localAndRemoteCommits.length > 0
			? [...localAndRemoteCommits.map(transformAnyCommit), { id: LineSpacer.LocalAndRemote }]
			: []
	);

	const lineManager = $derived(
		lineManagerFactory.build({
			remoteCommits: mappedRemoteCommits,
			localCommits: mappedLocalCommits,
			localAndRemoteCommits: mappedLocalAndRemoteCommits,
			integratedCommits: integratedCommits.map(transformAnyCommit)
		})
	);

	const hasCommits = $derived($branch.commits && $branch.commits.length > 0);
	const headCommit = $derived($branch.commits.at(0));

	const hasRemoteCommits = $derived(remoteCommits.length > 0);

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
	<div class="commits">
		<!-- UPSTREAM COMMITS -->

		{#if hasRemoteCommits}
			<StackingUpstreamCommitsAccordion count={Math.min(remoteCommits.length, 3)}>
				{#each remoteCommits as commit, idx (commit.id)}
					<StackingCommitCard
						type="remote"
						branch={$branch}
						{commit}
						{isUnapplied}
						last={idx === remoteCommits.length - 1}
						commitUrl={$gitHost?.commitUrl(commit.id)}
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
								await branchController.mergeUpstream($branch.id);
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
					<StackingCommitDragItem {commit}>
						<StackingCommitCard
							{commit}
							{isUnapplied}
							type="local"
							branch={$branch}
							last={idx === localCommits.length - 1}
							isHeadCommit={commit.id === headCommit?.id}
						>
							{#snippet lines()}
								<Line line={lineManager.get(commit.id)} />
							{/snippet}
						</StackingCommitCard>
					</StackingCommitDragItem>

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
					<StackingCommitDragItem {commit}>
						<StackingCommitCard
							{commit}
							{isUnapplied}
							type="localAndRemote"
							branch={$branch}
							last={idx === localAndRemoteCommits.length - 1}
							isHeadCommit={commit.id === headCommit?.id}
							commitUrl={$gitHost?.commitUrl(commit.id)}
						>
							{#snippet lines()}
								<Line line={lineManager.get(commit.id)} />
							{/snippet}
						</StackingCommitCard>
					</StackingCommitDragItem>
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
							<Line line={lineManager.get(LineSpacer.LocalAndRemote)} />
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
					<StackingCommitCard
						{commit}
						{isUnapplied}
						type="integrated"
						branch={$branch}
						isHeadCommit={commit.id === headCommit?.id}
						last={idx === integratedCommits.length - 1}
						commitUrl={$gitHost?.commitUrl(commit.id)}
					>
						{#snippet lines()}
							<Line line={lineManager.get(commit.id)} />
						{/snippet}
					</StackingCommitCard>
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
		border-radius: 0 0 var(--radius-m) var(--radius-m);
		overflow: hidden;
	}

	.commits-group {
		border-bottom: 1px solid var(--clr-border-2);

		&:last-child {
			border-bottom: none;
		}
	}
</style>
