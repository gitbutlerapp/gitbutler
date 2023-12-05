<script lang="ts">
	import { createEventDispatcher } from 'svelte';

	export let value: string | undefined;
	export let placeholder: string | undefined = undefined;
	export let required = false;
	export let rows = 4;
	export let id: string | undefined = undefined;

	const dispatch = createEventDispatcher<{ input: string; change: string }>();
</script>

<textarea
	class="text-box text-base-13"
	bind:value
	{id}
	{placeholder}
	{required}
	{rows}
	on:input={(e) => dispatch('input', e.currentTarget.value)}
	on:change={(e) => dispatch('change', e.currentTarget.value)}
/>

<style lang="postcss">
	.text-box {
		border-radius: var(--radius-s);
		border: 1px solid var(--clr-theme-container-outline-light);
		background: var(--clr-theme-container-light);

		display: flex;
		width: 100%;
		align-items: center;
		gap: var(--space-8);
		padding: var(--space-12);
		color: var(--clr-theme-scale-ntrl-0);

		resize: none;

		&::placeholder {
			/* Most modern browsers support this now. */
			color: var(--clr-theme-scale-ntrl-50);
		}

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
