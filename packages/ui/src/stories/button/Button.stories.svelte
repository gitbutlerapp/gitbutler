<script module lang="ts">
	import Button from '$lib/Button.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { fn, expect, userEvent, within } from '@storybook/test';

	const { Story } = defineMeta({
		title: 'Inputs / Button',
		component: Button,
		args: {
			loading: false,
			disabled: false,
			size: 'button',
			icon: 'ai-small',
			style: 'neutral',
			kind: 'solid',
			outline: false,
			dashed: false,
			solidBackground: false,
			helpShowDelay: 1200,
			id: 'button',
			tabindex: 0,
			type: 'button',
			shrinkable: false,
			reversedDirection: false,
			width: undefined,
			wide: false,
			grow: false,
			align: 'center',
			dropdownChild: false,
			onclick: fn(() => {
				console.log('Button clicked');
			})
		},
		argTypes: {
			size: {
				options: ['cta', 'button', 'tag'],
				control: {
					type: 'select'
				}
			}
		}
	});
</script>

<Story
	name="Playground"
	play={async ({ args, canvasElement }) => {
		const canvas = within(canvasElement);
		const submitButton = canvas.getByRole('button');
		await userEvent.click(submitButton);
		await expect(args.onclick).toHaveBeenCalled();
	}}
>
	Button
</Story>
