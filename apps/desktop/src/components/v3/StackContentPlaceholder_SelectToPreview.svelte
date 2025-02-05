<script lang="ts">
	import SelectACommit from '$lib/assets/illustrations/select-a-commit-preview.svg?raw';
	import { fade } from 'svelte/transition';
</script>

<div class="placeholder__svg" in:fade={{ duration: 250 }}>
	{@html SelectACommit}
</div>
<div class="placeholder__text">
	<div class="text-13 text-clr2">Select a commit to preview</div>
</div>

<style>
	.placeholder__svg {
		margin: 0 auto;
		height: 200px;
		width: 300px;
	}

	.placeholder__text {
		line-height: 1 !important;
		margin: 0 auto;

		white-space: pre-wrap;
		text-wrap: wrap;
	}
</style>
