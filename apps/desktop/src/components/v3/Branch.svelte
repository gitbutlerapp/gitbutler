<script lang="ts">
	import BranchDividerLine from './BranchDividerLine.svelte';
	import BranchCommitList from '$components/v3/BranchCommitList.svelte';
	import BranchHeader from '$components/v3/BranchHeader.svelte';
	import EmptyBranch from '$components/v3/EmptyBranch.svelte';
	import { isStackedBranch } from '$components/v3/lib';
	import type { WorkspaceBranch } from '$lib/branches/v3';

	interface Props {
		branch: WorkspaceBranch;
		first: boolean;
		last: boolean;
	}

	const { branch, first, last }: Props = $props();

	const localAndRemoteCommits = $derived(
		isStackedBranch(branch.state) ? branch.state.subject.localAndRemote : []
	);
	const upstreamOnlyCommits = $derived(
		isStackedBranch(branch.state) ? branch.state.subject.upstreamOnly : []
	);
</script>

{#if !first}
	<BranchDividerLine topPatchStatus={localAndRemoteCommits[0]?.state.type ?? 'Error'} />
{/if}
<div class="branch" data-series-name={branch.name}>
	<BranchHeader {branch} isTopBranch={first} />
	{#if !localAndRemoteCommits.length && !upstreamOnlyCommits.length}
		<EmptyBranch {last} />
	{/if}
	{#if isStackedBranch(branch.state)}
		<BranchCommitList commits={branch.state.subject} />
	{/if}
</div>

<style>
	.branch {
		position: relative;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: var(--clr-bg-1);
	}
</style>
