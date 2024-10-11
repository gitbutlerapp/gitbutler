import DemoIntegrationListItem from './DemoIntegrationListItem.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'List items / IntegrationListItem',
	component: DemoIntegrationListItem
} satisfies Meta<typeof DemoIntegrationListItem>;

export default meta;
type Story = StoryObj<typeof meta>;

export const BadgeStory: Story = {
	name: 'IntegrationListItem',
	args: {}
};
