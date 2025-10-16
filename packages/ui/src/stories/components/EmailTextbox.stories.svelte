<script module lang="ts">
	import EmailTextbox from '$components/EmailTextbox.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / EmailTextbox',
		component: EmailTextbox,
		args: {
			value: '',
			placeholder: 'Enter email address',
			disabled: false,
			readonly: false,
			required: false,
			wide: false,
			spellcheck: true,
			autocorrect: false,
			autocomplete: true,
			autofocus: false,
			selectall: false,
			label: '',
			helperText: '',
			customValidationMessage: 'Please enter a valid email address.',
			size: 'default',
			textAlign: 'left',
			iconLeft: undefined,
			iconRight: undefined,
			width: undefined
		},
		argTypes: {
			size: {
				control: { type: 'select' },
				options: ['default', 'large']
			},
			textAlign: {
				control: { type: 'select' },
				options: ['left', 'center', 'right']
			},
			iconLeft: {
				control: { type: 'text' }
			},
			iconRight: {
				control: { type: 'text' }
			}
		}
	});
</script>

<script lang="ts">
	let emailValue = $state('');
</script>

<Story name="Default">
	{#snippet template(args)}
		<div class="wrap">
			<EmailTextbox
				bind:value={emailValue}
				placeholder={args.placeholder}
				disabled={args.disabled}
				readonly={args.readonly}
				required={args.required}
				wide={args.wide}
				label={args.label}
				helperText={args.helperText}
				customValidationMessage={args.customValidationMessage}
				size={args.size}
				textAlign={args.textAlign}
				iconLeft={args.iconLeft}
				iconRight={args.iconRight}
				width={args.width}
			/>
		</div>
	{/snippet}
</Story>

<style>
	.wrap {
		max-width: 400px;
		padding: 16px;
	}
</style>
