import DemoSeriesLabelsRow from './DemoSeriesLabelsRow.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	title: 'Basic / Series Labels Row',
	component: DemoSeriesLabelsRow
} satisfies Meta<DemoSeriesLabelsRow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const DefaultStory: Story = {
	name: 'Series Labels Row',
	args: {
		series: [
			'feature/add-user-auth',
			'bugfix/fix-login-error',
			'hotfix/update-ssl-cert',
			'feature/improve-dashboard-ui',
			'release/v1.2.0',
			'feature/refactor-api-endpoints',
			'bugfix/remove-duplicate-entries',
			'chore/update-dependencies',
			'feature/add-password-reset',
			'hotfix/correct-typo-in-readme'
		],
		showRestAmount: true,
		selected: false
	}
};
