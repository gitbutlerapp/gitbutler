<script lang="ts">
	import Button from './Button.svelte';
	import { createEventDispatcher } from 'svelte';

	export let showFrame = false;

	const dispatch = createEventDispatcher();

	function show() {
		dispatch('show');
	}
</script>

<div class="large-diff-message" class:frame-box={showFrame}>
	<p class="text-base-13">Change hidden as large diffs may slow down the UI</p>

	<Button style="ghost" kind="solid" color="neutral" on:click={show}>Show anyways</Button>
</div>

<style>
	.large-diff-message {
		display: flex;
		padding: var(--size-12);
		gap: var(--size-8);
		flex-direction: column;
		background-color: var(--clr-container-light);
	}

	.frame-box {
		border: 1px solid var(--clr-container-outline-light);
		border-radius: var(--radius-m);
	}
</style>
