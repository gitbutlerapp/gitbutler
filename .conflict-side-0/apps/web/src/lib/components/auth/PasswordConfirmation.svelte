<script lang="ts">
	import { Textbox } from '@gitbutler/ui';

	interface Props {
		password?: string;
		passwordConfirmation?: string;
		showValidation?: boolean;
		autocomplete?: boolean;
	}

	let {
		password = $bindable(),
		passwordConfirmation = $bindable(),
		showValidation = true,
		autocomplete = true
	}: Props = $props();

	let passwordTouched = $state(false);
	let passwordConfirmationTouched = $state(false);

	const passwordsMatch = $derived(password === passwordConfirmation);

	function validatePassword(pwd: string) {
		if (!pwd) return { isValid: false, errors: [] };

		const errors = [];

		// Length check (minimum 8 characters)
		if (pwd.length < 8) {
			errors.push('at least 8 characters');
		}

		// Must contain at least one lowercase letter
		if (!/[a-z]/.test(pwd)) {
			errors.push('one lowercase letter');
		}

		// Must contain at least one uppercase letter
		if (!/[A-Z]/.test(pwd)) {
			errors.push('one uppercase letter');
		}

		// Must contain at least one number
		if (!/\d/.test(pwd)) {
			errors.push('one number');
		}

		return { isValid: errors.length === 0, errors };
	}

	const passwordValidation = $derived(validatePassword(password || ''));
	const isPasswordValid = $derived(passwordValidation.isValid);

	const passwordError = $derived(
		showValidation && passwordTouched && password && !isPasswordValid
			? `Password must contain: ${passwordValidation.errors.join(', ')}`
			: undefined
	);

	const passwordHelperText = $derived(
		showValidation && password && isPasswordValid
			? 'Strong password! ✅'
			: showValidation
				? '8+ characters with uppercase, lowercase, and number'
				: undefined
	);

	const passwordConfirmationError = $derived(
		passwordConfirmationTouched && passwordConfirmation && !passwordsMatch
			? 'Passwords do not match'
			: undefined
	);

	// Export validation state for parent components
	const _isValid = $derived(isPasswordValid && passwordConfirmation?.trim() && passwordsMatch);

	export function isValid() {
		return _isValid;
	}
</script>

<div class="password-confirmation">
	<Textbox
		bind:value={password}
		label="Password"
		type="password"
		{autocomplete}
		error={passwordError}
		helperText={passwordHelperText}
		onblur={() => {
			passwordTouched = true;
		}}
	/>
	<Textbox
		bind:value={passwordConfirmation}
		label="Confirm password"
		type="password-non-visible"
		{autocomplete}
		error={passwordConfirmationError}
		oninput={() => {
			passwordConfirmationTouched = true;
		}}
		onblur={() => {
			passwordConfirmationTouched = true;
		}}
	/>
</div>

<style lang="postcss">
	.password-confirmation {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}
</style>
