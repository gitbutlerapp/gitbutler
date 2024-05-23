<script lang="ts">
	import CommitCard from './CommitCard.svelte';
	import CommitLines from './CommitLines.svelte';
	import CommitListItem from './CommitListItem.svelte';
	import { getContextStore } from '$lib/utils/context';
	import {
		getLocalCommits,
		getRemoteCommits,
		getUnknownCommits,
		getUpstreamCommits
	} from '$lib/vbranches/contexts';
	import { BaseBranch, Branch } from '$lib/vbranches/types';

	export let isUnapplied: boolean;

	const branch = getContextStore(Branch);
	const localCommits = getLocalCommits();
	const remoteCommits = getRemoteCommits();
	const upstreamCommits = getUpstreamCommits();
	const unknownCommits = getUnknownCommits();
	const baseBranch = getContextStore(BaseBranch);

	$: hasShadowColumn = $localCommits.some((c) => c.relatedTo && c.id != c.relatedTo.id);
	$: hasLocalColumn = $localCommits.length > 0;
	$: hasCommits = $branch.commits && $branch.commits.length > 0;
	$: headCommit = $branch.commits.at(0);
	$: hasUnknownCommits = $unknownCommits.length > 0;
</script>

{#if hasCommits}
	<div class="commit-list__content">
		<div class="title text-base-13 text-semibold"></div>
		<div class="commits">
			{#if $unknownCommits.length > 0}
				<CommitLines {hasShadowColumn} {hasLocalColumn} localLine />
				{#each $unknownCommits as commit, idx (commit.id)}
					<div class="commit-lines">
						<CommitLines
							{hasLocalColumn}
							{hasShadowColumn}
							upstreamLine
							localLine={hasLocalColumn}
							remoteCommit={commit}
							first={idx == 0}
						/>
						<CommitListItem {commit}>
							<CommitCard
								type="upstream"
								branch={$branch}
								{commit}
								{isUnapplied}
								first={idx == 0}
								last={idx == $upstreamCommits.length - 1}
								commitUrl={$baseBranch?.commitUrl(commit.id)}
								isHeadCommit={commit.id === headCommit?.id}
							/>
						</CommitListItem>
					</div>
				{/each}
			{/if}
			{#if $localCommits.length > 0}
				<CommitLines
					{hasShadowColumn}
					{hasLocalColumn}
					upstreamLine={hasUnknownCommits}
					localLine
				/>
				{#each $localCommits as commit, idx (commit.id)}
					<div class="commit-lines">
						<CommitLines
							{hasLocalColumn}
							{hasShadowColumn}
							localCommit={commit}
							shadowLine={hasShadowColumn && !!commit.relatedTo}
							first={idx == 0}
							upstreamLine={hasUnknownCommits}
						/>
						<CommitListItem {commit}>
							<CommitCard
								branch={$branch}
								{commit}
								commitUrl={$baseBranch?.commitUrl(commit.id)}
								isHeadCommit={commit.id === headCommit?.id}
								{isUnapplied}
								first={idx == 0}
								last={idx == $localCommits.length - 1}
								type="local"
							/>
						</CommitListItem>
					</div>
				{/each}
			{/if}
			{#if $remoteCommits.length > 0}
				<CommitLines
					{hasShadowColumn}
					{hasLocalColumn}
					upstreamLine={hasUnknownCommits}
					localLine
				/>
				{#each $remoteCommits as commit, idx (commit.id)}
					<div class="commit-lines">
						<CommitLines
							{hasLocalColumn}
							{hasShadowColumn}
							localCommit={commit}
							localLine={idx == 0 && commit.parent?.status == 'local'}
							first={idx == 0}
							upstreamLine={hasUnknownCommits}
						/>
						<CommitListItem {commit}>
							<CommitCard
								branch={$branch}
								{commit}
								commitUrl={$baseBranch?.commitUrl(commit.id)}
								isHeadCommit={commit.id === headCommit?.id}
								{isUnapplied}
								first={idx == 0}
								last={idx == $remoteCommits.length - 1}
								type="remote"
							/>
						</CommitListItem>
					</div>
				{/each}
			{/if}
			<CommitLines
				{hasShadowColumn}
				localLine={$remoteCommits.length == 0 && $localCommits.length > 0}
				localRoot={$remoteCommits.length == 0 && $localCommits.length > 0}
				remoteLine={$remoteCommits.length > 0}
				shadowLine={hasShadowColumn}
				base
			/>
		</div>
	</div>
{/if}

<style lang="postcss">
	.commit-lines {
		display: flex;
	}

	.commit-list__content {
		display: flex;
		flex-direction: column;
	}

	.commits {
		display: flex;
		flex-direction: column;
	}
</style>
