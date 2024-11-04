<script lang="ts">
	import BranchPreviewHeader from '../branch/BranchPreviewHeader.svelte';
	import Resizer from '../shared/Resizer.svelte';
	import CommitCard from '$lib/commit/CommitCard.svelte';
	import { transformAnyCommit } from '$lib/commitLines/transformers';
	import Markdown from '$lib/components/Markdown.svelte';
	import FileCard from '$lib/file/FileCard.svelte';
	import { getForge } from '$lib/forge/interface/forge';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { RemoteBranchService } from '$lib/stores/remoteBranches';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { BranchData, type Branch } from '$lib/vbranches/types';
	import { getContext, getContextStoreBySymbol } from '@gitbutler/shared/context';
	import Line from '@gitbutler/ui/commitLines/Line.svelte';
	import { LineManagerFactory } from '@gitbutler/ui/commitLines/lineManager';
	import lscache from 'lscache';
	import { onMount, setContext } from 'svelte';
	import { writable } from 'svelte/store';
	import type { PullRequest } from '$lib/forge/interface/types';

	export let localBranch: Branch | undefined = undefined;
	export let remoteBranch: Branch | undefined = undefined;
	export let pr: PullRequest | undefined;

	const remoteBranchService = getContext(RemoteBranchService);
	const forge = getForge();

	const fileIdSelection = new FileIdSelection(writable([]));
	setContext(FileIdSelection, fileIdSelection);

	const selectedFile = fileIdSelection.selectedFile;
	$: commitId = $selectedFile?.commitId;
	$: selected = $selectedFile?.file;

	const defaultBranchWidthRem = 30;
	const laneWidthKey = 'branchPreviewLaneWidth';
	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	const lineManagerFactory = getContext(LineManagerFactory);

	let localBranchData: BranchData | undefined;
	let remoteBranchData: BranchData | undefined;

	// The remote branch service (which needs to be renamed) is responsible for
	// fetching local and remote branches.
	// We must manually set the branch data to undefined as the component
	// doesn't get completely re-rendered on a page change.
	$: if (localBranch) {
		remoteBranchService
			.getRemoteBranchData(localBranch.name)
			.then((branchData) => (localBranchData = branchData));
	} else {
		localBranchData = undefined;
	}

	$: if (remoteBranch) {
		remoteBranchService
			.getRemoteBranchData(remoteBranch.name)
			.then((branchData) => (remoteBranchData = branchData));
	} else {
		remoteBranchData = undefined;
	}

	$: remoteCommitShas = new Set(remoteBranchData?.commits.map((commit) => commit.id) || []);

	// Find commits common in the local and remote
	$: localAndRemoteCommits =
		localBranchData?.commits.filter((commit) => remoteCommitShas.has(commit.id)) || [];

	$: localAndRemoteCommitShas = new Set(localAndRemoteCommits.map((commit) => commit.id));

	// Find the local and remote commits that are not shared
	$: localCommits =
		localBranchData?.commits.filter((commit) => !localAndRemoteCommitShas.has(commit.id)) || [];
	$: remoteCommits =
		remoteBranchData?.commits.filter((commit) => !localAndRemoteCommitShas.has(commit.id)) || [];

	$: lineManager = lineManagerFactory.build(
		{
			remoteCommits: remoteCommits.map(transformAnyCommit),
			localCommits: localCommits.map(transformAnyCommit),
			localAndRemoteCommits: localAndRemoteCommits.map(transformAnyCommit),
			integratedCommits: []
		},
		true
	);

	let rsViewport: HTMLDivElement;
	let laneWidth: number;

	onMount(() => {
		laneWidth = lscache.get(laneWidthKey);
	});
</script>

{#if remoteBranch || localBranch}
	<div class="base">
		<div
			class="base__left"
			bind:this={rsViewport}
			style:width={`${laneWidth || defaultBranchWidthRem}rem`}
		>
			<ScrollableContainer wide>
				<div class="branch-preview">
					<BranchPreviewHeader {localBranch} {remoteBranch} {pr} />
					{#if pr}
						<div class="card">
							<div class="card__header text-14 text-body text-semibold">{pr.title}</div>
							{#if pr.body}
								<div class="card__content text-13 text-body">
									<Markdown content={pr.body} />
								</div>
							{/if}
						</div>
					{/if}
					<div class="branch-group">
						{#if remoteCommits}
							{#each remoteCommits as commit, index (commit.id)}
								<CommitCard
									isUnapplied
									last={index === remoteCommits.length - 1}
									{commit}
									commitUrl={$forge?.commitUrl(commit.id)}
									type="remote"
									disableCommitActions={true}
								>
									{#snippet lines()}
										<Line line={lineManager.get(commit.id)} />
									{/snippet}
								</CommitCard>
							{/each}
						{/if}
						{#if localCommits}
							{#each localCommits as commit, index (commit.id)}
								<CommitCard
									isUnapplied
									last={index === localCommits.length - 1}
									{commit}
									commitUrl={$forge?.commitUrl(commit.id)}
									type="local"
									disableCommitActions={true}
								>
									{#snippet lines()}
										<Line line={lineManager.get(commit.id)} />
									{/snippet}
								</CommitCard>
							{/each}
						{/if}
						{#if localAndRemoteCommits}
							{#each localAndRemoteCommits as commit, index (commit.id)}
								<CommitCard
									isUnapplied
									last={index === localAndRemoteCommits.length - 1}
									{commit}
									commitUrl={$forge?.commitUrl(commit.id)}
									type="localAndRemote"
									disableCommitActions={true}
								>
									{#snippet lines()}
										<Line line={lineManager.get(commit.id)} />
									{/snippet}
								</CommitCard>
							{/each}
						{/if}
					</div>
				</div>
			</ScrollableContainer>
			<Resizer
				viewport={rsViewport}
				direction="right"
				minWidth={320}
				on:width={(e) => {
					laneWidth = e.detail / (16 * $userSettings.zoom);
					lscache.set(laneWidthKey, laneWidth, 7 * 1440); // 7 day ttl
				}}
			/>
		</div>
		<div class="base__right">
			{#if selected}
				<FileCard
					conflicted={selected.conflicted}
					file={selected}
					isUnapplied={false}
					readonly={true}
					{commitId}
					on:close={() => {
						fileIdSelection.clear();
					}}
				/>
			{/if}
		</div>
	</div>
{:else}
	<p>No local or remote branch found</p>
{/if}

<style lang="postcss">
	.base {
		display: flex;
		width: 100%;
		overflow-x: auto;
	}
	.base__left {
		display: flex;
		flex-grow: 0;
		flex-shrink: 0;
		overflow-x: hidden;
		position: relative;
	}
	.base__right {
		display: flex;
		overflow-x: auto;
		align-items: flex-start;
		padding: 12px 12px 12px 6px;
		width: 800px;
	}

	.branch-preview {
		display: flex;
		flex-direction: column;
		gap: 8px;
		margin: 12px 6px 12px 12px;
	}

	.card__content {
		color: var(--clr-scale-ntrl-30);
	}

	.branch-group {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: var(--clr-bg-1);

		&:last-child {
			margin-bottom: 12px;
		}

		& :global(.commit-row):first-child {
			border-radius: var(--radius-m) var(--radius-m) 0 0;
		}
	}
</style>
