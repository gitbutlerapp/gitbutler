<script lang="ts">
	import CommitCard from './CommitCard.svelte';
	import CommitLines from './CommitLines.svelte';
	import { Project } from '$lib/backend/projects';
	import { getContext } from '$lib/utils/context';
	import { getContextStore } from '$lib/utils/context';
	import {
		getIntegratedCommits,
		getLocalCommits,
		getRemoteCommits,
		getUnknownCommits
	} from '$lib/vbranches/contexts';
	import { BaseBranch, Branch, Commit, type CommitStatus } from '$lib/vbranches/types';
	import { goto } from '$app/navigation';

	export let isUnapplied: boolean;

	const branch = getContextStore(Branch);
	const localCommits = getLocalCommits();
	const remoteCommits = getRemoteCommits();
	const unknownCommits = getUnknownCommits();
	const integratedCommits = getIntegratedCommits();
	const baseBranch = getContextStore(BaseBranch);
	const project = getContext(Project);

	$: hasShadowColumn =
		$integratedCommits.length == 0 &&
		$remoteCommits.length == 0 &&
		$localCommits.length > 0 &&
		$localCommits.at(0)?.relatedTo &&
		$localCommits.at(0)?.relatedTo?.id != $localCommits.at(0)?.id;
	$: hasLocalColumn = $localCommits.length > 0;
	$: hasCommits = $branch.commits && $branch.commits.length > 0;
	$: headCommit = $branch.commits.at(0);
	$: hasUnknownCommits = $unknownCommits.length > 0;
	$: hasShadowedCommits = $localCommits.some((c) => c.relatedTo);

	let baseIsUnfolded = false;

	function getNextUpstreamType(commit: Commit): CommitStatus | undefined {
		let child = commit.children?.[0];
		while (child) {
			if (child?.status == 'remote') return 'remote';
			child = child?.children?.[0];
		}
		if (hasUnknownCommits) return 'upstream';
	}
</script>

{#if hasCommits || hasUnknownCommits}
	<div class="commits">
		<!-- UPSTREAM COMMITS -->
		{#if $unknownCommits.length > 0}
			{#each $unknownCommits as commit, idx (commit.id)}
				<CommitCard
					type="upstream"
					branch={$branch}
					{commit}
					{isUnapplied}
					first={idx == 0}
					last={idx == $unknownCommits.length - 1}
					commitUrl={$baseBranch?.commitUrl(commit.id)}
					isHeadCommit={commit.id === headCommit?.id}
				>
					<svelte:fragment slot="lines">
						<CommitLines
							{hasLocalColumn}
							{hasShadowColumn}
							upstreamLine
							localLine={hasLocalColumn}
							remoteCommit={commit}
							first={idx == 0}
						/>
					</svelte:fragment>
				</CommitCard>
			{/each}
		{/if}
		<!-- LOCAL COMMITS -->
		{#if $localCommits.length > 0}
			{#each $localCommits as commit, idx (commit.id)}
				<CommitCard
					branch={$branch}
					{commit}
					commitUrl={$baseBranch?.commitUrl(commit.id)}
					isHeadCommit={commit.id === headCommit?.id}
					{isUnapplied}
					first={idx == 0}
					last={idx == $localCommits.length - 1}
					type="local"
				>
					<svelte:fragment slot="lines">
						<CommitLines
							{hasLocalColumn}
							{hasShadowColumn}
							localCommit={commit}
							shadowLine={hasShadowColumn && !!commit.relatedTo}
							first={idx == 0}
							upstreamLine={hasUnknownCommits}
							remoteLine={!hasShadowColumn && !!commit.relatedTo}
							upstreamType={getNextUpstreamType(commit)}
						/>
					</svelte:fragment>
				</CommitCard>
				<!-- </div> -->
			{/each}
		{/if}
		<!-- REMOTE COMMITS -->
		{#if $remoteCommits.length > 0}
			{#each $remoteCommits as commit, idx (commit.id)}
				<CommitCard
					branch={$branch}
					{commit}
					commitUrl={$baseBranch?.commitUrl(commit.id)}
					isHeadCommit={commit.id === headCommit?.id}
					{isUnapplied}
					first={idx == 0}
					last={idx == $remoteCommits.length - 1}
					type="remote"
				>
					<svelte:fragment slot="lines">
						<CommitLines
							remoteLine
							{hasLocalColumn}
							{hasShadowColumn}
							localCommit={commit}
							localLine={idx == 0 && commit.parent?.status == 'local'}
							first={idx == 0}
							upstreamLine={hasUnknownCommits}
							upstreamType={getNextUpstreamType(commit)}
						/>
					</svelte:fragment>
				</CommitCard>
			{/each}
		{/if}
		<!-- INTEGRATED COMMITS -->
		{#if $integratedCommits.length > 0}
			{#each $integratedCommits as commit, idx (commit.id)}
				<CommitCard
					branch={$branch}
					{commit}
					commitUrl={$baseBranch?.commitUrl(commit.id)}
					isHeadCommit={commit.id === headCommit?.id}
					{isUnapplied}
					first={idx == 0}
					last={idx == $integratedCommits.length - 1}
					type="integrated"
				>
					<svelte:fragment slot="lines">
						<CommitLines
							remoteLine
							{hasLocalColumn}
							{hasShadowColumn}
							localCommit={commit}
							localLine={idx == 0 && commit.parent?.status == 'local'}
							first={idx == 0}
							upstreamLine={$unknownCommits.length > 0 || $remoteCommits.length > 0}
							upstreamType={getNextUpstreamType(commit)}
						/>
					</svelte:fragment>
				</CommitCard>
			{/each}
		{/if}
		<!-- BASE -->
		<div class="base-row-container" class:base-row-container_unfolded={baseIsUnfolded}>
			<div
				class="commit-group base-row"
				tabindex="0"
				role="button"
				on:click|stopPropagation={() => (baseIsUnfolded = !baseIsUnfolded)}
				on:keydown={(e) => e.key === 'Enter' && (baseIsUnfolded = !baseIsUnfolded)}
			>
				<div class="base-row__lines">
					<CommitLines
						{hasShadowColumn}
						localLine={$remoteCommits.length == 0 && $localCommits.length > 0}
						localRoot={$remoteCommits.length == 0 &&
							$integratedCommits.length == 0 &&
							$localCommits.length > 0}
						remoteLine={$remoteCommits.length > 0 || $integratedCommits.length > 0}
						shadowLine={hasShadowColumn}
						{hasLocalColumn}
						upstreamType={hasShadowedCommits ? 'remote' : 'upstream'}
						base
						upstreamLine={hasUnknownCommits && $remoteCommits.length == 0}
					/>
				</div>
				<div class="base-row__content">
					<span class="text-base-11 base-row__text"
						>Base commit <button
							class="base-row__commit-link"
							on:click={async () => await goto(`/${project.id}/base`)}
						>
							{$branch.mergeBase ? $branch.mergeBase.slice(0, 7) : ''}
						</button>
					</span>
				</div>
			</div>
		</div>
	</div>
{/if}

<style lang="postcss">
	.commits {
		display: flex;
		flex-direction: column;
		background-color: var(--clr-bg-2);
		border-top: 1px solid var(--clr-border-2);
		border-bottom: 1px solid var(--clr-border-2);

		--base-top-margin: var(--size-8);
		--base-icon-top: var(--size-16);
		--base-unfolded: var(--size-48);

		--avatar-first-top: 3.1rem;
		--avatar-top: var(--size-16);
	}

	.commit-group {
		/* padding-right: var(--size-14);
		padding-left: var(--size-8); */
	}

	/* BASE ROW */

	.base-row-container {
		display: flex;
		flex-direction: column;
		height: var(--size-20);

		overflow: hidden;
		transition: height var(--transition-medium);
	}

	.base-row-container_unfolded {
		height: var(--base-unfolded);
		--base-icon-top: var(--size-20);

		& .base-row__text {
			opacity: 1;
		}
	}

	.base-row {
		display: flex;
		gap: var(--size-8);
		border-top: 1px solid var(--clr-border-3);
		min-height: calc(var(--base-unfolded) - var(--base-top-margin));
		margin-top: var(--base-top-margin);
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-2-muted);
		}
	}

	.base-row__lines {
		display: flex;
		margin-top: calc(var(--size-8) * -1);
	}

	.base-row__content {
		display: flex;
		align-items: center;
	}

	.base-row__text {
		color: var(--clr-text-2);
		opacity: 0;
		margin-top: var(--size-2);
		transition: opacity var(--transition-medium);
	}

	.base-row__commit-link {
		text-decoration: underline;
		cursor: pointer;
	}
</style>
