<script lang="ts">
	import { CommitSelection } from '$components/v3/selectedCommit.svelte';
	import { getContext } from 'svelte';

	let selectedCommit = getContext<CommitSelection>('_selectedCommit');
	$inspect('stackCommitDetails.selectedCommit', selectedCommit);
</script>

<div class="wrapper">
	<pre>commit: {selectedCommit.state}</pre>
</div>

<style>
	.wrapper {
		flex: 1;
		display: flex;
		flex-direction: column;
	}
</style>
