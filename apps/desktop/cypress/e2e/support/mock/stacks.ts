import type { Stack } from '$lib/stacks/stack';

export const MOCK_STACK_A: Stack = {
	id: '1234123',
	heads: [
		{
			name: 'branch-a',
			tip: '1234123'
		}
	],
	tip: '1234123'
};

export const MOCK_STACKS: Stack[] = [MOCK_STACK_A];
