<script lang="ts">
	import { getColorFromCommitState } from '$components/v3/lib';
	import type { Commit } from '$lib/branches/v3';

	interface Props {
		commit: Commit | null;
	}

	const { commit }: Props = $props();

	const lineColor = $derived(
		commit ? getColorFromCommitState(commit.id, commit.state) : 'var(--clr-commit-local)'
	);
</script>

<div class="commit-line" style:--commit-color={lineColor}></div>

<style>
	.commit-line {
		height: 10px;
		margin: 0 21px;
		background-color: var(--commit-color);
	}
</style>
