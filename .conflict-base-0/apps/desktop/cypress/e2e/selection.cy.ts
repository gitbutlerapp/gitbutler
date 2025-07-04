import { clearCommandMocks, mockCommand } from './support';
import { PROJECT_ID } from './support/mock/projects';
import BranchesWithChanges from './support/scenarios/branchesWithChanges';
import BranchesWithRemoteChanges from './support/scenarios/branchesWithRemoteChanges';

describe('Selection', () => {
	let mockBackend: BranchesWithChanges;

	beforeEach(() => {
		mockBackend = new BranchesWithChanges();

		mockCommand('stacks', () => mockBackend.getStacks());
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('changes_in_branch', (args) => mockBackend.getBranchChanges(args));
		mockCommand('hunk_assignments', (params) => mockBackend.getHunkAssignments(params));
		mockCommand('commit_details', (params) => mockBackend.getCommitChanges(params));
		mockCommand('tree_change_diffs', (params) => mockBackend.getDiff(params));

		cy.visit('/');

		cy.urlMatches(`/${PROJECT_ID}/workspace`);
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('should update the drawer when changing stack selection', () => {
		// Stack with branch should  be opened by default
		cy.getByTestId('branch-header').should('contain', mockBackend.stackId);

		const stacks = mockBackend.getStacks();
		// There shuold be three stacks
		cy.getByTestId('stack').should('have.length', stacks.length);

		// Select the second stack

		for (const stack of stacks) {
			const stackName = stack.heads[0]?.name;
			if (!stackName) continue;

			cy.getByTestIdByValue('branch-header', stackName)
				.click()
				.within(() => {
					// Shouls have the stack url
					cy.urlMatches(`/${PROJECT_ID}/workspace`);
				});
			// Check if the file list is updated
			cy.getByTestId('branch-view', stackName)
				.scrollIntoView()
				.should('be.visible')
				.within(() => {
					const changedFileNames = mockBackend.getBranchChangesFileNames(stack.id, stackName);
					for (const fileName of changedFileNames) {
						cy.getByTestId('file-list-item', fileName).should('be.visible');
					}
				});
		}
	});

	it('should be able to preview the files', () => {
		// Stack with branch should  be opened by default
		cy.getByTestId('branch-header').should('contain', mockBackend.stackId);

		const stacks = mockBackend.getStacks();
		// There shuold be three stacks
		cy.getByTestId('stack').should('have.length', stacks.length);

		// Select the initial commit which should be local only
		cy.getByTestId('commit-row', 'Initial commit').first().click();

		cy.getByTestId('commit-drawer')
			.should('be.visible')
			.within(() => {
				cy.getByTestId('commit-drawer-title').should('contain', 'Initial commit');
				cy.getByTestId('commit-drawer-description').should('contain', 'This is a test commit');
				cy.getByTestId('file-list-item', 'fileF.txt').should('be.visible');
				cy.getByTestId('file-list-item', 'fileE.txt').should('be.visible');
				cy.getByTestId('file-list-item', 'fileD.txt').should('be.visible').click();
			});

		cy.getByTestId('stack-selection-view').should('be.visible');
	});
});

describe('Selection with upstream changes', () => {
	let mockBackend: BranchesWithRemoteChanges;

	beforeEach(() => {
		mockBackend = new BranchesWithRemoteChanges();

		mockCommand('stacks', () => mockBackend.getStacks());
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('changes_in_branch', (args) => mockBackend.getBranchChanges(args));
		mockCommand('hunk_assignments', (params) => mockBackend.getHunkAssignments(params));

		cy.visit('/');

		cy.urlMatches(`/${PROJECT_ID}/workspace`);
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('should show the right context menu for the commit type', () => {
		// Stack with branch should  be opened by default
		cy.getByTestId('branch-header').should('contain', mockBackend.stackId);

		const stacks = mockBackend.getStacks();
		// There shuold be three stacks
		cy.getByTestId('stack').should('have.length', stacks.length);

		// Select the initial commit which should be local only
		cy.getByTestId('commit-row', 'Initial commit').first().rightclick();

		// Check if the commit context menu is shown
		cy.getByTestId('commit-row-context-menu').should('be.visible');

		// Upstream commit accordion should be visible
		cy.getByTestId('upstream-commits-accordion').should('be.visible').click();

		// Select the second commit which should be remote only
		cy.getByTestId('commit-row', 'Upstream commit 1').first().rightclick();

		// Check if the commit context menu is shown
		cy.getByTestId('commit-row-context-menu').should('not.exist');
	});
});
