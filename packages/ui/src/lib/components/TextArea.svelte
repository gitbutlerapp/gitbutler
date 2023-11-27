<script lang="ts">
	import { createEventDispatcher } from 'svelte';

	export let value: string;
	export let placeholder: string;
	export let required = false;
	export let rows = 4;

	const dispatch = createEventDispatcher<{ input: string }>();
</script>

<textarea
	class="text-box text-base-13"
	bind:value
	{placeholder}
	{required}
	{rows}
	on:input={(e) => dispatch('input', e.currentTarget.value)}
/>

<style lang="postcss">
	.text-box {
		border-radius: var(--radius-s);
		border: 1px solid var(--clr-theme-container-outline-light);
		background: var(--clr-theme-container-light);

		display: flex;
		width: 100%;
		color: var(--clr-theme-scale-ntrl-50);
		align-items: center;
		gap: var(--space-8);
		padding: var(--space-12);

		resize: none;

		&:hover {
			border-color: var(--clr-theme-container-outline-pale);
		}
		&:focus {
			border-color: var(--clr-theme-container-outline-sub);
		}
		&:invalid {
			border-color: var(--clr-theme-err-element);
		}
		&:disabled {
			color: var(--clr-theme-scale-ntrl-60);
			border-color: var(--clr-theme-err-element);
			background-color: var(--clr-theme-container-pale);
		}
	}
</style>
