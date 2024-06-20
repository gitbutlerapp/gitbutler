<script lang="ts">
	import { pxToRem } from '$lib/utils/pxToRem';
	import { useAutoHeight } from '$lib/utils/useAutoHeight';
	import { useResize } from '$lib/utils/useResize';
	import { createEventDispatcher } from 'svelte';

	export let value: string | undefined;
	export let placeholder: string | undefined = undefined;
	export let required = false;
	export let rows = 4;
	export let maxHeight: number | undefined = undefined;
	export let id: string | undefined = undefined;
	export let disabled = false;
	export let autocomplete: string | undefined = undefined;
	export let autocorrect: string | undefined = undefined;
	export let spellcheck = false;
	export let label: string | undefined = undefined;

	const dispatch = createEventDispatcher<{ input: string; change: string }>();

	let textareaElement: HTMLTextAreaElement;
</script>

<div class="textarea-wrapper">
	{#if label}
		<label class="textbox__label text-base-13 text-semibold" for={id}>
			{label}
		</label>
	{/if}
	<textarea
		bind:this={textareaElement}
		class="text-input text-base-body-13 textarea scrollbar"
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
		on:input={(e) => {
			dispatch('input', e.currentTarget.value);
			useAutoHeight(e.currentTarget);
		}}
		on:change={(e) => {
			dispatch('change', e.currentTarget.value);
			useAutoHeight(e.currentTarget);
		}}
		use:useResize={(e) => {
			useAutoHeight(e.currentTarget as HTMLTextAreaElement);
		}}
		on:focus={(e) => useAutoHeight(e.currentTarget)}
		style:max-height={maxHeight ? pxToRem(maxHeight) : undefined}
	></textarea>
</div>

<style lang="postcss">
	.textarea-wrapper {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.textarea {
		width: 100%;
		resize: none;
		padding: 12px;

		&::-webkit-resizer {
			background: transparent;
		}
	}

	.textbox__label {
		color: var(--clr-scale-ntrl-50);
	}
</style>
