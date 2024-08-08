import { spawn } from 'node:child_process';
import { browser } from '@wdio/globals';

describe('Project', () => {
	before(() => {
		// Use 'for-listing.sh' helper to generate dummy repositories for test

		// 4. Board
		const boardWorkspaceBtn = await $('button=Workspace');
		expect(boardWorkspaceBtn).toExist();
	});
});
