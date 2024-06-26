import DemoStaticCommitLines from './DemoStaticCommitLines.svelte';
import type { Meta, StoryObj } from '@storybook/svelte';

const meta = {
	component: DemoStaticCommitLines
} satisfies Meta<DemoStaticCommitLines>;

export default meta;
type Story = StoryObj<typeof meta>;

export const sameForkpoint: Story = {
	args: {
		lineGroups: [
			{
				lines: [
					{
						top: { type: 'straight', color: 'remote' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', color: 'remote' }
					},
					{
						top: { type: 'straight', color: 'local', style: 'dashed' },
						bottom: { type: 'straight', color: 'local', style: 'dashed' }
					},
					{
						top: { type: 'straight', color: 'none' },
						bottom: { type: 'straight', color: 'none' }
					}
				]
			},
			{
				lines: [
					{
						top: { type: 'straight', color: 'remote' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', color: 'remote' }
					},
					{
						top: { type: 'straight', color: 'local' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', color: 'local' }
					},
					{
						top: { type: 'straight', color: 'none' },
						bottom: { type: 'straight', color: 'none' }
					}
				]
			},
			{
				lines: [
					{
						top: { type: 'straight', color: 'remote' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', color: 'localAndRemote' }
					},
					{
						top: { type: 'fork', color: 'local' },
						bottom: { type: 'straight', color: 'none' }
					},
					{
						top: { type: 'straight', color: 'none' },
						bottom: { type: 'straight', color: 'none' }
					}
				]
			}
		]
	}
};

export const differentForkpoint: Story = {
	args: {
		lineGroups: [
			{
				lines: [
					{
						top: { type: 'straight', color: 'remote' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', color: 'remote' }
					},
					{
						top: { type: 'straight', color: 'none' },
						bottom: { type: 'straight', color: 'none' }
					},
					{
						top: { type: 'straight', color: 'local', style: 'dashed' },
						bottom: { type: 'straight', color: 'local', style: 'dashed' }
					},
					{
						top: { type: 'straight', color: 'none' },
						bottom: { type: 'straight', color: 'none' }
					}
				]
			},
			{
				lines: [
					{
						top: { type: 'straight', color: 'remote' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', color: 'shadow' }
					},
					{
						top: { type: 'straight', color: 'none' },
						bottom: { type: 'straight', color: 'none' }
					},
					{
						top: { type: 'straight', color: 'local', style: 'dashed' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', color: 'local' }
					},
					{
						top: { type: 'straight', color: 'none' },
						bottom: { type: 'straight', color: 'none' }
					}
				]
			},
			{
				lines: [
					{
						top: { type: 'straight', color: 'shadow' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', color: 'shadow' }
					},
					{
						top: { type: 'straight', color: 'none' },
						bottom: { type: 'straight', color: 'none' }
					},
					{
						top: { type: 'straight', color: 'local' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', color: 'local' }
					},
					{
						top: { type: 'straight', color: 'none' },
						bottom: { type: 'straight', color: 'none' }
					}
				]
			},
			{
				lines: [
					{
						top: { type: 'straight', color: 'shadow' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', color: 'shadow' }
					},
					{
						top: { type: 'straight', color: 'none' },
						bottom: { type: 'straight', color: 'none' }
					},
					{
						top: { type: 'straight', color: 'local' },
						commitNode: { type: 'large' },
						bottom: { type: 'fork', color: 'integrated' }
					},
					{
						top: { type: 'straight', color: 'none' },
						bottom: { type: 'straight', color: 'none' }
					}
				]
			}
		]
	}
};

export const onlyLocalAndRemote: Story = {
	args: {
		lineGroups: [
			{
				lines: [
					{
						top: { type: 'straight', color: 'localAndRemote' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', color: 'localAndRemote' }
					},
					{
						top: { type: 'straight', color: 'none' },
						bottom: { type: 'straight', color: 'none' }
					}
				]
			},
			{
				lines: [
					{
						top: { type: 'straight', color: 'localAndRemote' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', color: 'localAndRemote' }
					},
					{
						top: { type: 'straight', color: 'none' },
						bottom: { type: 'straight', color: 'none' }
					}
				]
			},
			{
				lines: [
					{
						top: { type: 'straight', color: 'localAndRemote' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', color: 'localAndRemote' }
					},
					{
						top: { type: 'straight', color: 'none' },
						bottom: { type: 'straight', color: 'none' }
					}
				]
			}
		]
	}
};
