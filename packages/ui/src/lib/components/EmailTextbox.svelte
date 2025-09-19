<script lang="ts">
	import Textbox from '$components/Textbox.svelte';

	interface Props {
		// Email-specific props
		customValidationMessage?: string;
		// All other props are forwarded to Textbox
		[key: string]: any;
	}

	let {
		customValidationMessage = 'Please enter a valid email address.',
		value = $bindable(),
		oninput,
		onchange,
		...restProps
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
	{...restProps}
	type="text"
	bind:value
	error={emailError}
	oninput={handleInput}
	onchange={handleChange}
/>
