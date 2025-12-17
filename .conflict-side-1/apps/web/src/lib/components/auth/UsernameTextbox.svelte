<script lang="ts">
	import { Textbox } from '@gitbutler/ui';

	interface Props {
		// Username-specific props
		customValidationMessage?: string;
		minLength?: number;
		maxLength?: number;
		value?: string;
		label?: string;
		placeholder?: string;
		// All other props are forwarded to Textbox
		[key: string]: any;
	}

	let {
		customValidationMessage = 'Please enter a valid username.',
		minLength = 3,
		maxLength = 30,
		value = $bindable(),
		label = 'Username',
		...restProps
	}: Props = $props();

	let usernameError = $state<string | undefined>(undefined);
	let usernameTouched = $state(false);

	function validateUsername(val: string): { isValid: boolean; message?: string } {
		if (!val) return { isValid: true }; // Empty is valid (unless required)

		// Check length
		if (val.length < minLength) {
			return {
				isValid: false,
				message: `Username must be at least ${minLength} characters long.`
			};
		}

		if (val.length > maxLength) {
			return {
				isValid: false,
				message: `Username must be no more than ${maxLength} characters long.`
			};
		}

		// Check for valid characters: alphanumeric, underscores, hyphens
		// Must start with alphanumeric character
		if (!/^[a-zA-Z0-9][a-zA-Z0-9_-]*$/.test(val)) {
			return {
				isValid: false,
				message:
					'Username must start with a letter or number and can only contain letters, numbers, underscores, and hyphens.'
			};
		}

		// Cannot end with hyphen or underscore
		if (/[-_]$/.test(val)) {
			return {
				isValid: false,
				message: 'Username cannot end with a hyphen or underscore.'
			};
		}

		// Cannot have consecutive special characters
		if (/[-_]{2,}/.test(val)) {
			return {
				isValid: false,
				message: 'Username cannot contain consecutive hyphens or underscores.'
			};
		}

		return { isValid: true };
	}

	function handleInput(val: string) {
		value = val;

		// Only show validation errors after the field has been touched (blurred once)
		if (usernameTouched) {
			const validation = validateUsername(val);
			usernameError =
				val && !validation.isValid ? validation.message || customValidationMessage : undefined;
		}
	}

	function handleChange() {
		// Mark as touched when user leaves the field
		usernameTouched = true;

		// Validate on blur
		if (value) {
			const validation = validateUsername(value);
			if (!validation.isValid) {
				usernameError = validation.message || customValidationMessage;
			} else {
				usernameError = undefined;
			}
		} else {
			usernameError = undefined;
		}
	}

	// Export validation state for parent components
	export function isValid(): boolean {
		if (!value) return true;
		return validateUsername(value).isValid;
	}

	export function validate(): boolean {
		usernameTouched = true;
		if (value) {
			const validation = validateUsername(value);
			if (!validation.isValid) {
				usernameError = validation.message || customValidationMessage;
				return false;
			} else {
				usernameError = undefined;
				return true;
			}
		} else {
			usernameError = undefined;
			return true;
		}
	}
</script>

<Textbox
	{...restProps}
	{label}
	type="text"
	bind:value
	error={usernameError}
	oninput={handleInput}
	onchange={handleChange}
/>
