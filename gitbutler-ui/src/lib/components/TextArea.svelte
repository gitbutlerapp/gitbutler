<script lang="ts">
	import { createEventDispatcher } from 'svelte';

	export let value: string | undefined;
	export let placeholder: string | undefined = undefined;
	export let required = false;
	export let rows = 4;
	export let id: string | undefined = undefined;
	export let disabled = false;
	export let autocomplete: string | undefined = undefined;
	export let autocorrect: string | undefined = undefined;
	export let spellcheck = false;
	export let label: string | undefined = undefined;

	const dispatch = createEventDispatcher<{ input: string; change: string }>();
</script>

<div class="textarea-wrapper">
	{#if label}
		<label class="textbox__label font-base-13 text-semibold" for={id}>
			{label}
		</label>
	{/if}
	<textarea
		class="text-input textarea"
		bind:value
		{disabled}
		{id}
		name={id}
		{placeholder}
		{required}
		{rows}
		{autocomplete}
		{autocorrect}
		{spellcheck}
		on:input={(e) => dispatch('input', e.currentTarget.value)}
		on:change={(e) => dispatch('change', e.currentTarget.value)}
	/>
</div>

<style lang="postcss">
	.textarea-wrapper {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
	}
	.textarea {
		width: 100%;
		resize: none;
		padding-top: var(--space-12);
		padding-bottom: var(--space-12);
	}

	.textbox__label {
		color: var(--clr-theme-scale-ntrl-50);
	}
</style>
