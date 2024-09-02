import DemoButton from './DemoButton.svelte';
import iconsJson from '$lib/data/icons.json';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Inputs / Button',
	component: DemoButton
} satisfies Meta<DemoButton>;

export default meta;
type Story = StoryObj<typeof meta>;

export const ButtonDefault: Story = {
	name: 'All Properties',
	args: {
		loading: false,
		disabled: false,
		clickable: true,
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
		onclick: () => {
			console.log('Button clicked');
		}
	},
	argTypes: {
		icon: { control: 'select', options: Object.keys(iconsJson) },
		type: { control: 'select', options: ['button', 'submit', 'reset'] },
		width: { control: 'text' },
		size: { control: 'select', options: ['tag', 'button', 'cta'] },
		align: {
			control: 'select',
			options: ['center', 'left', 'right', 'space-between']
		},
		style: {
			control: 'select',
			options: ['neutral', 'ghost', 'pop', 'success', 'error', 'warning', 'purple']
		},
		kind: { control: 'select', options: ['solid', 'soft'] }
	}
};

export const ButtonClickable: Story = {
	name: 'Not clickable + tooltip',
	args: {
		clickable: false,
		tooltip: 'This button is not clickable',
		tooltipAlign: 'start',
		onclick: () => {
			console.log('Button clicked');
		}
	},
	argTypes: {
		tooltipAlign: {
			control: 'select',
			options: ['start', 'center', 'end']
		}
	}
};
