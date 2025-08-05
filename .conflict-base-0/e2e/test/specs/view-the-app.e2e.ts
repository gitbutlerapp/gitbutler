import { startGitButler } from './utils';
import { expect, $ } from '@wdio/globals';
import * as path from 'node:path';

describe('Application starts', () => {
	beforeEach(async () => {});
	it('should start the application', async () => {
		const gitbutler = await startGitButler(browser);
		try {
			await gitbutler.runScript('setup-empty-project.sh');
			await gitbutler.visit('/');
			// Yay! Analytics prompt :D
			await expect($('.title')).toHaveText(expect.stringContaining('Before we begin'));

			await $('button.*=Continue').click();
			browser.setCookies({
				name: 'test-projectPath',
				value: path.resolve(gitbutler.workDir, 'local-clone')
			});
			await $('button.*=Add local project').click();
			await $('button.*=Continue').click();
			await $("button.*=Let's go!").click();

			await expect($('div.*=No branches in Workspace')).toBeDisplayed();
		} finally {
			await gitbutler.cleanup();
		}
	});
});
