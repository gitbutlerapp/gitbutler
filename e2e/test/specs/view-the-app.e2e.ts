import { startGitButler } from './utils';
import { expect, $ } from '@wdio/globals';

describe('Application starts', () => {
	beforeEach(async () => {});
	it('should start the application', async () => {
		const gitbutler = await startGitButler(browser);
		try {
			await gitbutler.visit('/');

			// Yay! Analytics prompt :D
			await expect($('.title')).toHaveText(expect.stringContaining('Before we begin'));
		} finally {
			await gitbutler.cleanup();
		}
	});
});
