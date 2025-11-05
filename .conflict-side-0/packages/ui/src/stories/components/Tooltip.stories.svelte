<script module lang="ts">
	import Tooltip from '$components/Tooltip.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Overlays / Tooltip',
		component: Tooltip,
		args: {
			text: 'This is a tooltip',
			align: 'center',
			position: 'top',
			disabled: false,
			hotkey: '⇧⌘K',
			children
		},
		argTypes: {
			text: {
				control: {
					type: 'text'
				}
			},
			align: {
				options: ['center', 'start', 'end'],
				control: {
					type: 'select'
				}
			},
			position: {
				options: ['top', 'bottom'],
				control: {
					type: 'select'
				}
			},
			disabled: {
				control: {
					type: 'boolean'
				}
			}
		}
	});
</script>

{#snippet children()}
	<span class="tooltip-text">tooltip</span>
{/snippet}

<Story name="default">
	{#snippet template(args)}
		<div class="wrapper">
			<p class="text-13 text">
				hello world! Here is a <Tooltip
					text={args.text}
					align={args.align}
					position={args.position}
					disabled={args.disabled}
					children={args.children}
					hotkey={args.hotkey}
				></Tooltip> for you.
			</p>
		</div>
	{/snippet}
</Story>

<Story name="Playground" />

<style>
	.wrapper {
		display: flex;
		flex-direction: column;
		padding: 30px;
	}

	.text {
		color: var(--clr-text-1);
	}

	.tooltip-text {
		text-decoration: underline;
		text-decoration-style: dotted;
	}
</style>
