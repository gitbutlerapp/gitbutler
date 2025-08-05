import { GitButler, startGitButler } from './utils';
import { expect, $ } from '@wdio/globals';
import * as path from 'node:path';

describe('Application starts', () => {
	let gitbutler: GitButler;

	beforeEach(async () => {
		gitbutler = await startGitButler(browser);
	});

	it('should start the application', async () => {
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
	});
	after(async () => {
		await gitbutler.cleanup();
	});
});
