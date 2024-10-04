<script lang="ts">
	import { autoSelectBranchNameFeature } from '$lib/config/uiFeatureFlags';
	import { resizeObserver } from '$lib/utils/resizeObserver';

	interface Props {
		value: string;
		disabled?: boolean;
		onChange?: (value: string) => void;
		placeholder?: string;
		class?: string;
		multiline?: boolean;
		inputEl?: HTMLInputElement | HTMLTextAreaElement;
	}

	let {
		value,
		placeholder,
		disabled = false,
		onChange,
		inputEl = $bindable(),
		class: className = '',
		multiline = false
	}: Props = $props();

	let initialValue = value;
	let textAreaWidth = $state('');
	let inputWidth = $state('');
	let inputHeight = $state('');
</script>

<span
	use:resizeObserver={(e) => {
		inputWidth = `${Math.round(e.frame.width)}px`;
		inputHeight = `${Math.round(e.frame.height)}px`;
	}}
	class="label-input-measure-el"
	class:text-12={multiline}
	class:text-14={!multiline}
	class:wrap={multiline}
	style:width={multiline ? textAreaWidth : 'inherit'}
>
	{value}
</span>
{#if multiline}
	<textarea
		{disabled}
		bind:this={inputEl}
		bind:value
		onchange={(e) => onChange?.(e.currentTarget.value.trim())}
		title={value}
		{placeholder}
		class={`label-input ${className}`}
		ondblclick={(e) => e.stopPropagation()}
		onclick={(e) => {
			e.stopPropagation();
			inputEl?.focus();
			if ($autoSelectBranchNameFeature) {
				inputEl?.select();
			}
		}}
		onblur={() => {
			if (value === '') value = initialValue;
		}}
		onfocus={() => {
			initialValue = value;
		}}
		onkeydown={(e) => {
			if ((e.key === 'Enter' && !e.shiftKey) || e.key === 'Escape') {
				inputEl?.blur();
			}
		}}
		use:resizeObserver={(e) => {
			textAreaWidth = `${Math.round(e.frame.width)}px`;
		}}
		autocomplete="off"
		autocorrect="off"
		spellcheck="false"
		style:height={inputHeight}
	></textarea>
{:else}
	<input
		type="text"
		{disabled}
		bind:this={inputEl}
		bind:value
		onchange={(e) => onChange?.(e.currentTarget.value.trim())}
		title={value}
		{placeholder}
		class={`label-input ${className}`}
		ondblclick={(e) => e.stopPropagation()}
		onclick={(e) => {
			e.stopPropagation();
			inputEl?.focus();
			if ($autoSelectBranchNameFeature) {
				inputEl?.select();
			}
		}}
		onblur={() => {
			if (value === '') value = initialValue;
		}}
		onfocus={() => {
			initialValue = value;
		}}
		onkeydown={(e) => {
			if (e.key === 'Enter' || e.key === 'Escape') {
				inputEl?.blur();
			}
		}}
		autocomplete="off"
		autocorrect="off"
		spellcheck="false"
		style:width={inputWidth}
	/>
{/if}

<style lang="postcss">
	.label-input-measure-el,
	.label-input {
		min-width: 44px;
		padding: 2px 4px;
		min-height: 20px;
		border: 1px solid transparent;
	}
	.label-input-measure-el {
		pointer-events: none;
		visibility: hidden;
		border: 1px solid transparent;
		color: black;
		position: fixed;
		display: inline-block;
		white-space: pre;

		&.wrap {
			white-space: pre-wrap;
		}
	}
	.label-input {
		text-overflow: ellipsis;

		width: 100%;
		border-radius: var(--radius-s);
		color: var(--clr-scale-ntrl-0);
		background-color: var(--clr-bg-1);
		outline: none;

		/* not readonly */
		&:not([disabled]):hover {
			background-color: var(--clr-bg-2);
		}

		&:not([disabled]):focus {
			outline: none;
			background-color: var(--clr-bg-2);
			border-color: var(--clr-border-2);
		}
	}

	input {
		height: 20px;
		overflow: hidden;
		white-space: nowrap;
	}

	textarea {
		max-height: 76px;
		resize: none;
		word-break: break-all;
		overflow-wrap: break-word;
		overflow-x: hidden;
	}
</style>
