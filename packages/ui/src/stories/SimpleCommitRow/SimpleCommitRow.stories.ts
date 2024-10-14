import DemoSimpleCommitRow from './DemoSimpleCommitRow.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta: Meta = {
	title: 'List items / Simple Commit Row',
	component: DemoSimpleCommitRow
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Story: Story = {
	name: 'Simple Commit Row',
	args: {}
};
