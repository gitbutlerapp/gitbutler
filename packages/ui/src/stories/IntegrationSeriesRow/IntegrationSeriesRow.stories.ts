import DemoIntegrationSeriesRow from './DemoIntegrationSeriesRow.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta: Meta = {
	title: 'List items / Integration Series Row',
	component: DemoIntegrationSeriesRow
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Story: Story = {
	name: 'Integration Series Row',
	args: {}
};
