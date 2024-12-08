import { spawnAndLog, findAndClick, setElementValue } from '../utils.js';

const TEMP_DIR = '/tmp/gitbutler-add-project';
const REPO_NAME = 'one-vbranch-on-integration';

describe('Project', () => {
	before(() => {
		spawnAndLog('bash', [
			'-c',
			`
				source ./e2e/scripts/init.sh ${TEMP_DIR}
				cd ${TEMP_DIR};
				git clone remote ${REPO_NAME} && cd ${REPO_NAME}
				$CLI project -s dev add --switch-to-workspace "$(git rev-parse --symbolic-full-name "@{u}")"
				$CLI branch create virtual
			`
		]);
	});

	it('should add a local project', async () => {
		await findAndClick('button[data-testid="analytics-continue"]');

		const dirInput = await $('input[data-testid="test-directory-path"]');
		setElementValue(dirInput, `${TEMP_DIR}/${REPO_NAME}`);

		await $('button[data-testid="add-local-project"]').then(async (b) => await b.click());
		await $('button[data-testid="set-base-branch"]').then(async (b) => await b.click());
		await $('button[data-testid="accept-git-auth"]').then(async (b) => await b.click());

		await expect($('button=Workspace')).toExist();
	});
});
