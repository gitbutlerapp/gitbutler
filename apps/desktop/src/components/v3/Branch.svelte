<script lang="ts">
	import BranchCommitList from '$components/v3/BranchCommitList.svelte';
	import BranchHeader from '$components/v3/BranchHeader.svelte';
	import type { WorkspaceBranch } from '$lib/branches/v3';

	interface Props {
		branch: WorkspaceBranch;
		first: boolean;
		last: boolean;
	}

	const { branch, first }: Props = $props();
</script>

<div class="branch" data-series-name={branch.name}>
	<BranchHeader {branch} isTopBranch={first} />
	<div>
		{#if branch.state.type !== 'Archived' && branch.state.subject.upstreamOnly.length}
			<BranchCommitList commits={branch.state.subject.upstreamOnly} />
		{/if}
		{#if branch.state.type !== 'Archived' && branch.state.subject.localAndRemote.length}
			<BranchCommitList commits={branch.state.subject.localAndRemote} />
		{/if}
	</div>
</div>

<style>
	.branch {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: var(--clr-bg-1);
	}
</style>
