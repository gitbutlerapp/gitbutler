import { clearCommandMocks, mockCommand } from './support';
import LotsOfFileChanges from './support/scenarios/lotsOfFileChanges';

describe('File Tree', () => {
	let mockBackend: LotsOfFileChanges;
	beforeEach(() => {
		mockBackend = new LotsOfFileChanges();
		mockCommand('stacks', () => mockBackend.getStacks());
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('changes_in_worktree', (params) => mockBackend.getWorktreeChanges(params));

		cy.visit('/');

		cy.url({ timeout: 3000 }).should('include', `/workspace/${mockBackend.stackId}`);
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('Should be able to toggle the file tree view - Uncommitted changes', () => {
		// There should be uncommitted changes
		cy.getByTestId('uncommitted-changes-file-list').should('be.visible');

		// All files should be visible
		cy.getByTestId('uncommitted-changes-file-list-item').should(
			'have.length',
			mockBackend.getWorktreeChangesFileNames().length
		);

		// The uncommitted changes header should be visible,
		// and the file list mode should be selected
		cy.getByTestId('uncommitted-changes-header')
			.should('be.visible')
			.within(() => {
				cy.get('#list').should('be.visible').should('have.attr', 'aria-selected', 'true');

				// Click the tree view button
				cy.get('#tree').should('be.visible').should('have.attr', 'aria-selected', 'false').click();
			});

		// There should be collapsed file tree
		cy.getByTestId('uncommitted-changes-file-list').within(() => {
			cy.getByTestId('file-list-tree-folder').should(
				'have.length',
				mockBackend.getWorktreeChangesTopLevelDirs().length
			);

			cy.getByTestId('uncommitted-changes-file-list-item').should(
				'have.length',
				mockBackend.getWorktreeChangesTopLevelFiles().length
			);
		});
	});
});
