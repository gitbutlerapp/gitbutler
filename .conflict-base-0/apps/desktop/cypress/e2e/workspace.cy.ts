import { clearCommandMocks } from './support';
import { PROJECT_ID } from './support/mock/projects';

describe('Workspace', () => {
	beforeEach(() => {});

	afterEach(() => {
		clearCommandMocks();
	});

	it('Should be redirected to the workspace', () => {
		// This is basically just a smoke test to check that the workspace is loaded
		cy.visit('/');

		// Should be redirected to the workspac
		cy.urlMatches(`/${PROJECT_ID}/workspace`);
	});
});
