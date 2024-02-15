<script lang="ts">
	import { createEventDispatcher } from 'svelte';

	export let value: string | undefined;
	export let placeholder: string | undefined = undefined;
	export let required = false;
	export let rows = 4;
	export let id: string | undefined = undefined;
	export let disabled = false;
	export let spellcheck = false;

	export let kind: 'default' | 'plain' = 'default';

	const dispatch = createEventDispatcher<{ input: string; change: string }>();
</script>

<textarea
	class="textarea text-base-13"
	class:default={kind == 'default'}
	bind:value
	{disabled}
	{id}
	{placeholder}
	{required}
	{rows}
	{spellcheck}
	on:input={(e) => dispatch('input', e.currentTarget.value)}
	on:change={(e) => dispatch('change', e.currentTarget.value)}
/>

<style lang="postcss">
	.textarea {
		width: 100%;
		color: var(--clr-theme-scale-ntrl-0);
		outline: none;
		resize: none;
		background-color: transparent;

		&::placeholder {
			/* Most modern browsers support this now. */
			color: var(--clr-theme-scale-ntrl-50);
		}

		&:disabled {
			color: var(--clr-theme-scale-ntrl-60);
		}
	}
	.default {
		background: var(--clr-theme-container-light);
		padding: var(--space-12);
		border: 1px solid var(--clr-theme-container-outline-light);
		border-radius: var(--radius-s);

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
			background-color: var(--clr-theme-container-pale);
			border-color: var(--clr-theme-err-element);
		}
	}
</style>
