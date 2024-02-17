<script lang="ts">
	import { createEventDispatcher } from 'svelte';

	export let value: string | undefined;
	export let placeholder: string | undefined = undefined;
	export let required = false;
	export let rows = 4;
	export let id: string | undefined = undefined;
	export let disabled = false;
	export let spellcheck = false;

	const dispatch = createEventDispatcher<{ input: string; change: string }>();
</script>

<textarea
	class="text-input textarea"
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
		resize: none;
		padding-top: var(--space-12);
		padding-bottom: var(--space-12);
	}
</style>
