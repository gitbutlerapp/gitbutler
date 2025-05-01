<script lang="ts">
	import BranchPreviewHeader from '$components/BranchPreviewHeader.svelte';
	import CommitCard from '$components/CommitCard.svelte';
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import FileCard from '$components/FileCard.svelte';
	import Resizer from '$components/Resizer.svelte';
	import { BranchData } from '$lib/branches/branch';
	import { transformAnyCommit } from '$lib/commits/transformers';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { FileIdSelection } from '$lib/selection/fileIdSelection';
	import { getContext } from '@gitbutler/shared/context';
	import { persistWithExpiration } from '@gitbutler/shared/persisted';
	import Line from '@gitbutler/ui/commitLines/Line.svelte';
	import { LineManagerFactory } from '@gitbutler/ui/commitLines/lineManager';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';
	import { setContext } from 'svelte';
	import type { PullRequest } from '$lib/forge/interface/types';

	interface Props {
		projectId: string;
		localBranch?: BranchData | undefined;
		remoteBranch?: BranchData | undefined;
		pr: PullRequest | undefined;
	}

	const { projectId, localBranch = undefined, remoteBranch = undefined, pr }: Props = $props();

	const forge = getContext(DefaultForgeFactory);

	const fileIdSelection = new FileIdSelection();
	setContext(FileIdSelection, fileIdSelection);

	const selectedFile = fileIdSelection.selectedFile;
	const commitId = $derived($selectedFile?.commitId);
	const selected = $derived($selectedFile?.file);

	const width = persistWithExpiration<number>(30, 'branchPreviewLaneWidth', 7 * 1440);
	const lineManagerFactory = getContext(LineManagerFactory);

	const remoteCommitShas = $derived(
		new Set(remoteBranch?.commits.map((commit) => commit.id) || [])
	);

	// Find commits common in the local and remote
	const localAndRemoteCommits = $derived(
		localBranch?.commits.filter((commit) => remoteCommitShas.has(commit.id)) || []
	);

	const localAndRemoteCommitShas = $derived(
		new Set(localAndRemoteCommits.map((commit) => commit.id))
	);

	// Find the local and remote commits that are not shared
	const localCommits = $derived(
		localBranch?.commits.filter((commit) => !localAndRemoteCommitShas.has(commit.id)) || []
	);
	const remoteCommits = $derived(
		remoteBranch?.commits.filter((commit) => !localAndRemoteCommitShas.has(commit.id)) || []
	);

	const lineManager = $derived(
		lineManagerFactory.build({
			remoteCommits: remoteCommits.map(transformAnyCommit),
			localCommits: localCommits.map(transformAnyCommit),
			localAndRemoteCommits: localAndRemoteCommits.map(transformAnyCommit),
			integratedCommits: []
		})
	);

	let rsViewport = $state<HTMLDivElement>();
</script>

{#if remoteBranch || localBranch}
	<div class="base">
		<div class="base__left" bind:this={rsViewport} style:width={$width + 'rem'}>
			<ScrollableContainer wide>
				<div class="branch-preview">
					<BranchPreviewHeader {projectId} {localBranch} {remoteBranch} {pr} />
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
									{projectId}
									isUnapplied
									last={index === remoteCommits.length - 1}
									{commit}
									commitUrl={forge.current.commitUrl(commit.id)}
									type="Remote"
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
									{projectId}
									isUnapplied
									last={index === localCommits.length - 1}
									{commit}
									commitUrl={forge.current.commitUrl(commit.id)}
									type="LocalOnly"
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
									{projectId}
									isUnapplied
									last={index === localAndRemoteCommits.length - 1}
									{commit}
									commitUrl={forge.current.commitUrl(commit.id)}
									type="LocalAndRemote"
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
				minWidth={20}
				onWidth={(value) => ($width = value)}
			/>
		</div>
		<div class="base__right">
			{#if selected}
				<FileCard
					file={selected}
					isUnapplied={false}
					readonly={true}
					{commitId}
					onClose={() => {
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
