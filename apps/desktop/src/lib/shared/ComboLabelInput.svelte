<script lang="ts">
	import { autoSelectBranchNameFeature } from '$lib/config/uiFeatureFlags';
	import TextInput from '$lib/shared/TextInput.svelte';
	import { debounce } from '$lib/utils/debounce';
	import { resizeObserver } from '$lib/utils/resizeObserver';

	interface Props {
		value: string;
		disabled?: boolean;
		onChange?: (value: string) => void;
		placeholder?: string;
		class?: string;
		multiline?: boolean;
	}

	let {
		value,
		placeholder,
		disabled = false,
		onChange,
		class: className = '',
		multiline = false
	}: Props = $props();

	let initialValue = value;
	// `textAreaWidth` is required for multiline because the hidden observer div is
	// positioned  'fixed' and so it won't stop growing with its content when it hits the
	// end/edge of its container, and in the case of multiline, it must wrap the text at
	// the same time as the actual textarea in order to keep the height/width we're measuring
	// correct
	let textAreaWidth = $state('');
	let inputWidth = $state('');
	let inputHeight = $state('');
	let inputEl = $state<HTMLTextAreaElement | HTMLInputElement>();
</script>

<span
	use:resizeObserver={debounce((e) => {
		inputWidth = `${Math.round(e.frame.width)}px`;
		inputHeight = `${Math.round(e.frame.height)}px`;
	}, 100)}
	class="label-input-measure-el"
	class:text-12={multiline}
	class:text-14={!multiline}
	class:text-bold={!multiline}
	class:wrap={multiline}
	style:width={multiline ? textAreaWidth : 'auto'}
>
	{value}
</span>
<TextInput
	{multiline}
	{placeholder}
	{disabled}
	bind:element={inputEl}
	bind:value
	bind:textAreaWidth
	bind:inputHeight
	bind:inputWidth
	title={value}
	autocomplete="off"
	autocorrect="off"
	spellcheck="false"
	class={`label-input ${className}`}
	onchange={(e) => onChange?.(e.currentTarget.value.trim())}
	ondblclick={(e: MouseEvent) => e.stopPropagation()}
	onclick={(e: MouseEvent) => {
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
	onkeydown={(e: KeyboardEvent) => {
		if ((e.key === 'Enter' && !e.shiftKey) || e.key === 'Escape') {
			inputEl?.blur();
		}
	}}
/>

<style lang="postcss">
	.label-input-measure-el {
		position: fixed;
		display: inline-block;
		visibility: hidden;
		min-width: 44px;
		min-height: 20px;

		padding: 2px 4px;
		pointer-events: none;
		color: black;
		border: 2px solid transparent;
		white-space: pre;

		&.wrap {
			white-space: pre-wrap;
		}
	}
</style>
