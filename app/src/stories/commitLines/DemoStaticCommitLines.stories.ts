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
						top: { type: 'straight', style: 'remote' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', style: 'remote' }
					},
					{
						top: { type: 'straight', style: 'localDashed' },
						bottom: { type: 'straight', style: 'localDashed' }
					},
					{
						top: { type: 'straight', style: 'none' },
						bottom: { type: 'straight', style: 'none' }
					}
				]
			},
			{
				lines: [
					{
						top: { type: 'straight', style: 'remote' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', style: 'remote' }
					},
					{
						top: { type: 'straight', style: 'local' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', style: 'local' }
					},
					{
						top: { type: 'straight', style: 'none' },
						bottom: { type: 'straight', style: 'none' }
					}
				]
			},
			{
				lines: [
					{
						top: { type: 'straight', style: 'remote' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', style: 'localAndRemote' }
					},
					{
						top: { type: 'fork', style: 'local' },
						bottom: { type: 'straight', style: 'none' }
					},
					{
						top: { type: 'straight', style: 'none' },
						bottom: { type: 'straight', style: 'none' }
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
						top: { type: 'straight', style: 'remote' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', style: 'remote' }
					},
					{
						top: { type: 'straight', style: 'none' },
						bottom: { type: 'straight', style: 'none' }
					},
					{
						top: { type: 'straight', style: 'localDashed' },
						bottom: { type: 'straight', style: 'localDashed' }
					},
					{
						top: { type: 'straight', style: 'none' },
						bottom: { type: 'straight', style: 'none' }
					}
				]
			},
			{
				lines: [
					{
						top: { type: 'straight', style: 'remote' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', style: 'shadow' }
					},
					{
						top: { type: 'straight', style: 'none' },
						bottom: { type: 'straight', style: 'none' }
					},
					{
						top: { type: 'straight', style: 'localDashed' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', style: 'local' }
					},
					{
						top: { type: 'straight', style: 'none' },
						bottom: { type: 'straight', style: 'none' }
					}
				]
			},
			{
				lines: [
					{
						top: { type: 'straight', style: 'shadow' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', style: 'shadow' }
					},
					{
						top: { type: 'straight', style: 'none' },
						bottom: { type: 'straight', style: 'none' }
					},
					{
						top: { type: 'straight', style: 'local' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', style: 'local' }
					},
					{
						top: { type: 'straight', style: 'none' },
						bottom: { type: 'straight', style: 'none' }
					}
				]
			},
			{
				lines: [
					{
						top: { type: 'straight', style: 'shadow' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', style: 'shadow' }
					},
					{
						top: { type: 'straight', style: 'none' },
						bottom: { type: 'straight', style: 'none' }
					},
					{
						top: { type: 'straight', style: 'local' },
						commitNode: { type: 'large' },
						bottom: { type: 'fork', style: 'integrated' }
					},
					{
						top: { type: 'straight', style: 'none' },
						bottom: { type: 'straight', style: 'none' }
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
						top: { type: 'straight', style: 'localAndRemote' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', style: 'localAndRemote' }
					},
					{
						top: { type: 'straight', style: 'none' },
						bottom: { type: 'straight', style: 'none' }
					}
				]
			},
			{
				lines: [
					{
						top: { type: 'straight', style: 'localAndRemote' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', style: 'localAndRemote' }
					},
					{
						top: { type: 'straight', style: 'none' },
						bottom: { type: 'straight', style: 'none' }
					}
				]
			},
			{
				lines: [
					{
						top: { type: 'straight', style: 'localAndRemote' },
						commitNode: { type: 'large' },
						bottom: { type: 'straight', style: 'localAndRemote' }
					},
					{
						top: { type: 'straight', style: 'none' },
						bottom: { type: 'straight', style: 'none' }
					}
				]
			}
		]
	}
};
