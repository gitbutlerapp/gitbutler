import DemoButton from './ButtonDemo.svelte';
import iconsJson from '$lib/icon/icons.json';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	component: DemoButton,
	argTypes: {
		disabled: {
			control: 'boolean'
		},
		clickable: {
			control: 'boolean'
		},
		loading: {
			control: 'boolean'
		},
		size: {
			control: 'select',
			options: ['tag', 'button', 'cta']
		},
		style: {
			control: 'select',
			options: ['neutral', 'ghost', 'pop', 'success', 'error', 'warning', 'purple', undefined]
		},
		kind: {
			control: 'select',
			options: ['solid', 'soft', undefined]
		},
		outline: {
			control: 'boolean'
		},
		dashed: {
			control: 'boolean'
		},
		icon: {
			control: 'select',
			options: [undefined, ...Object.keys(iconsJson)]
		}
	}
} satisfies Meta<DemoButton>;

export default meta;
type Story = StoryObj<typeof meta>;

export const ButtonDefalut: Story = {
	args: {
		contents: 'Testeroni',
		size: 'button',
		disabled: false,
		clickable: true,
		loading: false,
		style: 'neutral',
		kind: 'soft',
		outline: false,
		dashed: false,
		icon: undefined
	}
};

export const ButtonWithIcon: Story = {
    args: {
        contents: "Testeroni",
        size: "button",
        disabled: false,
        clickable: true,
        loading: false,
        style: "pop",
        kind: "solid",
        outline: false,
        dashed: false,
        icon: "ai-small"
    }
};
