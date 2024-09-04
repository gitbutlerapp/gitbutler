import DemoInfoButton from './DemoInfoButton.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Overlays / InfoButton',
	component: DemoInfoButton
} as Meta<DemoInfoButton>;

export default meta;
type Story = StoryObj<typeof meta> & {
	args: {
		headerContent: string;
		bodyContent: string;
		linkAddress: string;
		linkText: string;
	};
	argTypes: {
		headerContent: { control: 'text' };
		bodyContent: { control: 'text' };
		linkAddress: { control: 'text' };
		linkText: { control: 'text' };
	};
};

export const DefaultStory: Story = {
	name: 'InfoButton',
	args: {
		headerContent: 'My Info Button',
		bodyContent: 'This is a fantastic info button :D',
		linkAddress: 'https://google.com',
		linkText: 'Google'
	},
	argTypes: {
		headerContent: { control: 'text' },
		bodyContent: { control: 'text' },
		linkAddress: { control: 'text' },
		linkText: { control: 'text' }
	}
};
