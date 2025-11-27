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

<Story name="Multiple Tooltips">
	{#snippet template()}
		<div class="wrapper">
			<p class="text-13 text">Hover over these items quickly to test instant tooltips:</p>
			<div class="tooltip-grid">
				<Tooltip text="First tooltip">
					<button type="button" class="test-button">Item 1</button>
				</Tooltip>
				<Tooltip text="Second tooltip">
					<button type="button" class="test-button">Item 2</button>
				</Tooltip>
				<Tooltip text="Third tooltip">
					<button type="button" class="test-button">Item 3</button>
				</Tooltip>
				<Tooltip text="Fourth tooltip">
					<button type="button" class="test-button">Item 4</button>
				</Tooltip>
				<Tooltip text="Fifth tooltip">
					<button type="button" class="test-button">Item 5</button>
				</Tooltip>
				<Tooltip text="Sixth tooltip">
					<button type="button" class="test-button">Item 6</button>
				</Tooltip>
			</div>
			<p class="text-13 text" style="margin-top: 20px;">
				The first tooltip has a 500ms delay. Once shown, move between items quickly - subsequent
				tooltips appear instantly without delay or animation.
			</p>
		</div>
	{/snippet}
</Story>

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

	.tooltip-grid {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		margin-top: 20px;
		gap: 16px;
	}

	.test-button {
		padding: 12px 24px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: var(--clr-bg-1);
		color: var(--clr-text-1);
		cursor: pointer;
		transition: all 0.15s ease;
	}

	.test-button:hover {
		border-color: var(--clr-border-3);
		background: var(--clr-bg-2);
	}
</style>
