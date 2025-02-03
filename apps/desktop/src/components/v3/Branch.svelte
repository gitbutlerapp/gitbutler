<script lang="ts">
	import BranchHeader from '$components/v3/BranchHeader.svelte';
	import type { WorkspaceBranch } from '$lib/branches/v3';
	// import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';

	interface Props {
		branch: WorkspaceBranch;
		first: boolean;
		last: boolean;
	}

	const { branch, first, last }: Props = $props();

	// - If any of the commits are integrated; whole branch is considered "integrated"
	// - branch status comes from 1st commit (local only / local and remote / etc.)
</script>

<div class="branch-group" data-series-name={branch.name}>
	<!-- style:--highlight-color={getColorFromBranchType(seriesType)} -->
	<BranchHeader {branch} isTopBranch={first} />
	<pre>
		<!-- {JSON.stringify(branch, null, 2)} -->
	</pre>
</div>

<style>
	.branch-group {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: var(--clr-bg-1);
		scroll-margin-top: 120px;

		&:last-child {
			margin-bottom: 12px;
		}
	}
</style>
