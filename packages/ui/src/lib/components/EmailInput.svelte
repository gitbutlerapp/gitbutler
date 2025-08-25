<script lang="ts">
	import Textbox from '$components/Textbox.svelte';
	import type iconsJson from '$lib/data/icons.json';
	import type { Snippet } from 'svelte';

	interface Props {
		element?: HTMLElement;
		id?: string;
		testId?: string;
		iconLeft?: keyof typeof iconsJson;
		iconRight?: keyof typeof iconsJson;
		customIconLeft?: Snippet;
		customIconRight?: Snippet;
		size?: 'default' | 'large';
		textAlign?: 'left' | 'center' | 'right';
		value?: string;
		width?: number;
		placeholder?: string;
		helperText?: string;
		label?: string;
		wide?: boolean;
		disabled?: boolean;
		readonly?: boolean;
		required?: boolean;
		selectall?: boolean;
		spellcheck?: boolean;
		autocorrect?: boolean;
		autocomplete?: boolean;
		autofocus?: boolean;
		onclick?: (e: MouseEvent & { currentTarget: EventTarget & HTMLInputElement }) => void;
		onmousedown?: (e: MouseEvent & { currentTarget: EventTarget & HTMLInputElement }) => void;
		oninput?: (val: string) => void;
		onchange?: (val: string) => void;
		onkeydown?: (e: KeyboardEvent & { currentTarget: EventTarget & HTMLInputElement }) => void;
		// Email-specific props
		customValidationMessage?: string;
	}

	let {
		element = $bindable(),
		id,
		testId,
		iconLeft,
		iconRight,
		customIconLeft,
		customIconRight,
		size = 'default',
		textAlign = 'left',
		value = $bindable(),
		width,
		placeholder = 'Enter email address',
		helperText,
		label,
		wide,
		disabled,
		readonly,
		required,
		selectall,
		spellcheck,
		autocorrect,
		autocomplete,
		autofocus,
		onclick,
		onmousedown,
		oninput,
		onchange,
		onkeydown,
		customValidationMessage = 'Please enter a valid email address.'
	}: Props = $props();

	let emailError = $state<string | undefined>(undefined);
	let emailTouched = $state(false);

	function validateEmail(val: string): boolean {
		if (!val) return true; // Empty is valid (unless required)
		// Simple email regex
		return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(val);
	}

	function handleInput(val: string) {
		value = val;
		oninput?.(val);

		// Only show validation errors after the field has been touched (blurred once)
		if (emailTouched) {
			emailError = val && !validateEmail(val) ? customValidationMessage : undefined;
		}
	}

	function handleChange() {
		// Mark as touched when user leaves the field
		emailTouched = true;

		// Validate on blur
		if (value && !validateEmail(value)) {
			emailError = customValidationMessage;
		} else {
			emailError = undefined;
		}

		onchange?.(value || '');
	}

	// Export validation state for parent components
	export function isValid(): boolean {
		return !value || validateEmail(value);
	}

	export function validate(): boolean {
		emailTouched = true;
		if (value && !validateEmail(value)) {
			emailError = customValidationMessage;
			return false;
		} else {
			emailError = undefined;
			return true;
		}
	}
</script>

<Textbox
	bind:element
	{id}
	{testId}
	type="text"
	{iconLeft}
	{iconRight}
	{customIconLeft}
	{customIconRight}
	{size}
	{textAlign}
	bind:value
	{width}
	{placeholder}
	{helperText}
	error={emailError}
	{label}
	{wide}
	{disabled}
	{readonly}
	{required}
	{selectall}
	{spellcheck}
	{autocorrect}
	{autocomplete}
	{autofocus}
	{onclick}
	{onmousedown}
	oninput={handleInput}
	onchange={handleChange}
	{onkeydown}
/>
