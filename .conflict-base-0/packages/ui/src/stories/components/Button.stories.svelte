<script module lang="ts">
	import Button from '$components/Button.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / Button',
		component: Button,
		args: {
			children: 'Button Text' as any,
			disabled: false,
			loading: false,
			activated: false,
			size: 'button',
			style: 'neutral',
			kind: 'solid',
			wide: false,
			grow: false,
			shrinkable: false,
			reversedDirection: false,
			dropdownChild: false,
			hotkey: '',
			tooltip: '',
			icon: undefined,
			onclick: (e: MouseEvent) => {
				// eslint-disable-next-line no-console
				console.log('Button clicked', e);
			}
		}
	});
</script>

<Story name="Playground">
	{#snippet template(args)}
		<Button
			disabled={args.disabled}
			loading={args.loading}
			activated={args.activated}
			size={args.size}
			style={args.style}
			kind={args.kind}
			wide={args.wide}
			grow={args.grow}
			shrinkable={args.shrinkable}
			reversedDirection={args.reversedDirection}
			dropdownChild={args.dropdownChild}
			hotkey={args.hotkey}
			tooltip={args.tooltip}
			icon={args.icon}
			onclick={args.onclick}
		>
			{args.children}
		</Button>
	{/snippet}
</Story>
