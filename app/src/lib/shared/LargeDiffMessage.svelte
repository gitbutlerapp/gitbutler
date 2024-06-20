<script lang="ts">
	import Button from '$lib/shared/Button.svelte';
	import { createEventDispatcher } from 'svelte';

	export let showFrame = false;

	const dispatch = createEventDispatcher();

	function show() {
		dispatch('show');
	}
</script>

<div class="large-diff-message" class:frame-box={showFrame}>
	<p class="text-base-13">Change hidden as large diffs may slow down the UI</p>

	<Button style="ghost" outline on:click={show}>Show anyways</Button>
</div>

<style>
	.large-diff-message {
		display: flex;
		padding: 12px;
		gap: 8px;
		flex-direction: column;
		background-color: var(--clr-bg-1);
	}

	.frame-box {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}
</style>
