import { clearCommandMocks, mockCommand } from './support';
import { PROJECT_ID } from './support/mock/projects';
import LotsOfFileChanges from './support/scenarios/lotsOfFileChanges';
import SomeDeeplyNestedChanges from './support/scenarios/someDeeplyNestedChanges';

describe('File Tree - multiple file changes', () => {
	let mockBackend: LotsOfFileChanges;
	beforeEach(() => {
		mockBackend = new LotsOfFileChanges();
		mockCommand('stacks', () => mockBackend.getStacks());
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('changes_in_worktree', (params) => mockBackend.getWorktreeChanges(params));
		mockCommand('hunk_assignments', (params) => mockBackend.getHunkAssignments(params));
		mockCommand('list_workspace_rules', (params) => mockBackend.listWorkspaceRules(params));
		mockCommand('get_author_info', (params) => mockBackend.getAuthorInfo(params));

		cy.visit('/');

		cy.urlMatches(`/${PROJECT_ID}/workspace`);
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('Should be able to toggle the file tree view - Uncommitted changes', () => {
		// There should be uncommitted changes
		cy.getByTestId('uncommitted-changes-file-list').should('be.visible');

		// All files should be visible
		cy.getByTestId('file-list-item').should(
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
	});
});

describe('File Tree - some file changes', () => {
	let mockBackend: SomeDeeplyNestedChanges;
	beforeEach(() => {
		mockBackend = new SomeDeeplyNestedChanges();
		mockCommand('stacks', () => mockBackend.getStacks());
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('changes_in_worktree', (params) => mockBackend.getWorktreeChanges(params));
		mockCommand('hunk_assignments', (params) => mockBackend.getHunkAssignments(params));
		mockCommand('list_workspace_rules', (params) => mockBackend.listWorkspaceRules(params));
		mockCommand('get_author_info', (params) => mockBackend.getAuthorInfo(params));

		cy.visit('/');

		cy.urlMatches(`/${PROJECT_ID}/workspace`);
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('Should be able to toggle the file tree view - Uncommitted changes', () => {
		// There should be uncommitted changes
		cy.getByTestId('uncommitted-changes-file-list').should('be.visible');

		// All files should be visible
		cy.getByTestId('file-list-item').should(
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

		// There should be an expanded file tree
		cy.getByTestId('uncommitted-changes-file-list').within(() => {
			cy.getByTestId('file-list-tree-folder').should(
				'have.length',
				mockBackend.getWorktreeChangesTopLevelDirs().length
			);

			cy.getByTestId('file-list-item').should(
				'have.length',
				mockBackend.getWorktreeChangesFileNames().length
			);
		});
	});

	it('Correctly remembers the file tree view state', () => {
		// There should be uncommitted changes
		cy.getByTestId('uncommitted-changes-file-list').should('be.visible');

		// All files should be visible
		cy.getByTestId('file-list-item').should(
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

		// There should be an expanded file tree
		cy.getByTestId('uncommitted-changes-file-list').within(() => {
			cy.getByTestId('file-list-tree-folder').should(
				'have.length',
				mockBackend.getWorktreeChangesTopLevelDirs().length
			);

			cy.getByTestId('file-list-item').should(
				'have.length',
				mockBackend.getWorktreeChangesFileNames().length
			);
		});

		// Click on the branches view
		cy.getByTestId('navigation-branches-button').should('be.visible').click();

		// Click the workspace button
		cy.getByTestId('navigation-workspace-button').should('be.visible').click();

		// There should be an expanded file tree
		cy.getByTestId('uncommitted-changes-file-list').within(() => {
			cy.getByTestId('file-list-tree-folder').should(
				'have.length',
				mockBackend.getWorktreeChangesTopLevelDirs().length
			);

			cy.getByTestId('file-list-item').should(
				'have.length',
				mockBackend.getWorktreeChangesFileNames().length
			);
		});

		// The file tree view should still be selected
		cy.getByTestId('uncommitted-changes-header')
			.should('be.visible')
			.within(() => {
				cy.get('#tree').should('be.visible').should('have.attr', 'aria-selected', 'true');
			});
	});
});
