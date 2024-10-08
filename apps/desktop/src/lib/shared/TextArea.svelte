<script lang="ts">
	import { autoHeight } from '@gitbutler/ui/utils/autoHeight';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	import { resizeObserver } from '@gitbutler/ui/utils/resizeObserver';
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
		<label class="textbox__label text-13 text-semibold" for={id}>
			{label}
		</label>
	{/if}
	<textarea
		bind:this={textareaElement}
		class="text-input text-13 text-body textarea scrollbar"
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
			autoHeight(e.currentTarget);
		}}
		on:change={(e) => {
			dispatch('change', e.currentTarget.value);
			autoHeight(e.currentTarget);
		}}
		use:resizeObserver={(e) => {
			autoHeight(e.currentTarget as HTMLTextAreaElement);
		}}
		on:focus={(e) => autoHeight(e.currentTarget)}
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
