<script lang="ts">
	import { autoHeight } from '$lib/utils/autoHeight';
	import { resizeObserver } from '$lib/utils/resizeObserver';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	import { createEventDispatcher } from 'svelte';

	interface Props {
		value: string | undefined;
		placeholder?: string | undefined;
		required?: boolean;
		rows?: number;
		maxHeight?: number | undefined;
		id?: string | undefined;
		disabled?: boolean;
		autocomplete?: string | undefined;
		autocorrect?: string | undefined;
		spellcheck?: boolean;
		label?: string | undefined;
	}

	let {
		value = $bindable(),
		placeholder = undefined,
		required = false,
		rows = 4,
		maxHeight = undefined,
		id = undefined,
		disabled = false,
		autocomplete = undefined,
		autocorrect = undefined,
		spellcheck = false,
		label = undefined
	}: Props = $props();

	const dispatch = createEventDispatcher<{ input: string; change: string }>();

	let textareaElement: HTMLTextAreaElement = $state();
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
		oninput={(e) => {
			dispatch('input', e.currentTarget.value);
			autoHeight(e.currentTarget);
		}}
		onchange={(e) => {
			dispatch('change', e.currentTarget.value);
			autoHeight(e.currentTarget);
		}}
		use:resizeObserver={(e) => {
			autoHeight(e.currentTarget as HTMLTextAreaElement);
		}}
		onfocus={(e) => autoHeight(e.currentTarget)}
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
