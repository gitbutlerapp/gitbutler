import { clearCommandMocks } from './support';
import { PROJECT_ID } from './support/mock/projects';

describe('Workspace', () => {
	beforeEach(() => {});

	afterEach(() => {
		clearCommandMocks();
	});

	it('Should be redirected to the workspace', () => {
		// This is basically jsut a smoke test to check that the workspace is loaded
		cy.visit('/');

		// Should be redirected to the workspac
		cy.url({ timeout: 3000 }).should('include', `/${PROJECT_ID}/workspace`);
	});
});
