<script lang="ts">
	import CloudPatchStackIndex from '@gitbutler/shared/cloud/stacks/CloudPatchStackIndex.svelte';
</script>

<div class="series-container">
	<h2 class="text-head-24 heading">Your patch stacks:</h2>
	<CloudPatchStackIndex />
</div>

<style lang="postcss">
	.heading {
		margin-bottom: 16px;
	}

	.series-container {
		display: flex;
		flex-direction: column;

		max-width: 600px;
		width: 100%;

		margin: 24px auto;
	}
</style>
