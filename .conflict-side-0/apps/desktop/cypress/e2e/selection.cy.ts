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
		mockCommand('commit_details_with_line_stats', (params) => mockBackend.getCommitChanges(params));
		mockCommand('tree_change_diffs', (params) => mockBackend.getDiff(params));
		mockCommand('list_workspace_rules', (params) => mockBackend.listWorkspaceRules(params));
		mockCommand('get_author_info', (params) => mockBackend.getAuthorInfo(params));

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
		// There should be three stacks
		cy.getByTestId('stack').should('have.length', stacks.length);

		// Select the second stack

		for (const stack of stacks) {
			const stackName = stack.heads[0]?.name;
			const stackId = stack.id!;
			if (!stackName) continue;

			cy.getByTestIdByValue('branch-header', stackName)
				.click()
				.within(() => {
					// Should have the stack url
					cy.urlMatches(`/${PROJECT_ID}/workspace`);
				});
			// Check if the file list is updated
			cy.getByTestIdByValue('stack', stackId)
				.scrollIntoView()
				.should('be.visible')
				.within(() => {
					const branch = `refs/heads/${stackName}`;
					const changedFileNames = mockBackend.getBranchChangesFileNames(stackId, branch);
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
		const stack = stacks[0]!;
		// There should be three stacks
		cy.getByTestId('stack').should('have.length', stacks.length);

		// Select the initial commit which should be local only
		cy.getByTestId('commit-row', 'Initial commit').first().click();

		cy.getByTestIdByValue('stack', stack.id!)
			.should('be.visible')
			.within(() => {
				cy.getByTestId('commit-drawer-title').should('contain', 'Initial commit');
				cy.getByTestId('commit-drawer-description').should('contain', 'This is a test commit');
				cy.getByTestId('file-list-item', 'fileA.ts').should('be.visible').click();
				cy.getByTestId('file-list-item', 'fileB.txt').should('be.visible').click();
				cy.getByTestId('file-list-item', 'fileC.txt').should('be.visible').click();
			});

		cy.getByTestId('stack-selection-view').should('be.visible');
	});

	it('should be update the commit drawer when changing the commit selection', () => {
		// Stack with branch should  be opened by default
		cy.getByTestId('branch-header').should('contain', mockBackend.stackWithTwoCommits);

		const stacks = mockBackend.getStacks();
		// There should be three stacks
		cy.getByTestId('stack').should('have.length', stacks.length);

		cy.getByTestIdByValue('stack', mockBackend.stackWithTwoCommits)
			.should('be.visible')
			.within(() => {
				// Select the first commit
				cy.getByTestId('commit-row').should('have.length', 2).first().click();
				cy.getByTestId('commit-drawer-title').should('contain', 'Second commit');
				cy.getByTestId('commit-drawer-description').should(
					'contain',
					'This is another test commit'
				);
				cy.getByTestId('file-list-item', 'fileD.txt').should('be.visible');

				// Select the second commit
				cy.getByTestId('commit-row').should('have.length', 2).last().click();
				cy.getByTestId('commit-drawer-title').should('contain', 'Also second commit');
				cy.getByTestId('commit-drawer-description').should(
					'contain',
					'This is another test commit, but with a different title'
				);
				cy.getByTestId('file-list-item', 'fileE.txt').should('be.visible');
				cy.getByTestId('file-list-item', 'fileF.txt').should('be.visible');

				// Select the first commit again
				cy.getByTestId('commit-row').should('have.length', 2).first().click();
				cy.getByTestId('commit-drawer-title').should('contain', 'Second commit');
				cy.getByTestId('commit-drawer-description').should(
					'contain',
					'This is another test commit'
				);
				cy.getByTestId('file-list-item', 'fileD.txt').should('be.visible');
				cy.getByTestId('file-list-item', 'fileE.txt').should('not.exist');
				cy.getByTestId('file-list-item', 'fileF.txt').should('not.exist');

				// Select the branch header
				cy.getByTestId('branch-header').click();
				cy.getByTestId('commit-drawer').should('not.exist');
				cy.getByTestId('file-list-item', 'fileD.txt').should('be.visible');
				cy.getByTestId('file-list-item', 'fileE.txt').should('be.visible');
				cy.getByTestId('file-list-item', 'fileF.txt').should('be.visible');

				// Select the second commit again
				cy.getByTestId('commit-row').should('have.length', 2).last().click();
				cy.getByTestId('commit-drawer-title').should('contain', 'Also second commit');
				cy.getByTestId('commit-drawer-description').should(
					'contain',
					'This is another test commit, but with a different title'
				);
				cy.getByTestId('file-list-item', 'fileE.txt').should('be.visible');
				cy.getByTestId('file-list-item', 'fileF.txt').should('be.visible');
				cy.getByTestId('file-list-item', 'fileD.txt').should('not.exist');
			});
	});

	it('should be update the commit drawer when changing the commit selection', () => {
		// Stack with branch should  be opened by default
		cy.getByTestId('branch-header').should('contain', mockBackend.stackWithTwoCommits);

		const stacks = mockBackend.getStacks();
		// There should be three stacks
		cy.getByTestId('stack').should('have.length', stacks.length);

		cy.getByTestIdByValue('stack', mockBackend.stackWithTwoCommits)
			.should('be.visible')
			.within(() => {
				// Select the first commit
				cy.getByTestId('commit-row').should('have.length', 2).first().click();
				cy.getByTestId('commit-drawer-title').should('contain', 'Second commit');
				cy.getByTestId('commit-drawer-description').should(
					'contain',
					'This is another test commit'
				);
				cy.getByTestId('file-list-item', 'fileD.txt').should('be.visible');

				// Select the second commit
				cy.getByTestId('commit-row').should('have.length', 2).last().click();
				cy.getByTestId('commit-drawer-title').should('contain', 'Also second commit');
				cy.getByTestId('commit-drawer-description').should(
					'contain',
					'This is another test commit, but with a different title'
				);
				cy.getByTestId('file-list-item', 'fileE.txt').should('be.visible');
				cy.getByTestId('file-list-item', 'fileF.txt').should('be.visible');

				// Select the first commit again
				cy.getByTestId('commit-row').should('have.length', 2).first().click();
				cy.getByTestId('commit-drawer-title').should('contain', 'Second commit');
				cy.getByTestId('commit-drawer-description').should(
					'contain',
					'This is another test commit'
				);
				cy.getByTestId('file-list-item', 'fileD.txt').should('be.visible');
				cy.getByTestId('file-list-item', 'fileE.txt').should('not.exist');
				cy.getByTestId('file-list-item', 'fileF.txt').should('not.exist');

				// Select the branch header
				cy.getByTestId('branch-header').click();
				cy.getByTestId('commit-drawer').should('not.exist');
				cy.getByTestId('file-list-item', 'fileD.txt').should('be.visible');
				cy.getByTestId('file-list-item', 'fileE.txt').should('be.visible');
				cy.getByTestId('file-list-item', 'fileF.txt').should('be.visible');

				// Select the second commit again
				cy.getByTestId('commit-row').should('have.length', 2).last().click();
				cy.getByTestId('commit-drawer-title').should('contain', 'Also second commit');
				cy.getByTestId('commit-drawer-description').should(
					'contain',
					'This is another test commit, but with a different title'
				);
				cy.getByTestId('file-list-item', 'fileF.txt').should('be.visible');
				cy.getByTestId('file-list-item', 'fileD.txt').should('not.exist');

				// Open a file
				cy.getByTestId('file-list-item', 'fileE.txt').should('be.visible').click();
			});
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
		mockCommand('list_workspace_rules', (params) => mockBackend.listWorkspaceRules(params));
		mockCommand('get_author_info', (params) => mockBackend.getAuthorInfo(params));

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
		// There should be three stacks
		cy.getByTestId('stack').should('have.length', stacks.length);

		// Select the initial commit which should be local only
		cy.getByTestId('commit-row', 'Initial commit').first().rightclick();

		// Check if the commit context menu is shown
		cy.getByTestId('commit-row-context-menu').should('be.visible');

		// Upstream integration button should be visible (all upstream commits are shown by default)
		cy.getByTestId('upstream-commits-integrate-button').should('be.visible');

		// Select the second commit which should be remote only
		cy.getByTestId('commit-row', 'Upstream commit 1').first().rightclick();

		// Check if the commit context menu is shown
		cy.getByTestId('commit-row-context-menu').should('not.exist');
	});
});
