<script module lang="ts">
	import Tooltip from '$lib/Tooltip.svelte';
	import {
		type Args,
		defineMeta,
		setTemplate,
		type StoryContext
	} from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Overlays / Tooltip',
		component: Tooltip,
		args: {
			text: 'This is a tooltip',
			align: 'center',
			position: 'top',
			disabled: false
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

<script lang="ts">
	setTemplate(template);
</script>

{#snippet template({ ...args }: Args<typeof Story>, _context: StoryContext<typeof Story>)}
	<div class="wrapper">
		<p class="text-13 text">
			hello world! Here is a <Tooltip {...args}>
				<span class="tooltip-text">tooltip</span>
			</Tooltip> for you.
		</p>
	</div>
{/snippet}

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
