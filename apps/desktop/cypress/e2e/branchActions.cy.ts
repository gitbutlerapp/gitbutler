import { clearCommandMocks, mockCommand } from './support';
import { PROJECT_ID } from './support/mock/projects';
import BranchesWithRemoteChanges from './support/scenarios/branchesWithRemoteChanges';

describe('Branch Actions', () => {
	let mockBackend: BranchesWithRemoteChanges;

	beforeEach(() => {
		mockBackend = new BranchesWithRemoteChanges();

		mockCommand('stacks', () => mockBackend.getStacks());
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('changes_in_branch', (args) => mockBackend.getBranchChanges(args));
		mockCommand('integrate_upstream_commits', (args) => mockBackend.integrateUpstreamCommits(args));
		mockCommand('update_branch_name', (params) => mockBackend.renameBranch(params));
		mockCommand('remove_branch', (params) => mockBackend.removeBranch(params));
		mockCommand('create_branch', (params) => mockBackend.addBranch(params));
		mockCommand('hunk_assignments', (params) => mockBackend.getHunkAssignments(params));

		cy.visit('/');

		cy.urlMatches(`/${PROJECT_ID}/workspace`);
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('should be able to integrate upstream commits of a branch', () => {
		// spies
		cy.spy(mockBackend, 'integrateUpstreamCommits').as('integrateUpstreamCommits');

		// the upstream commits accordion should be visible
		cy.getByTestId('upstream-commits-accordion').should('be.visible').click();

		// The integrate button should be visible
		cy.getByTestId('upstream-commits-integrate-button').should('be.visible').click();

		// The accordion should be closed
		cy.getByTestId('upstream-commits-accordion').should('not.exist');

		// The commits should be integrated
		cy.getByTestId('stack', mockBackend.stackId)
			.should('exist')
			.within(() => {
				cy.getByTestId('commit-row').should('have.length', 5);
			});

		cy.get('@integrateUpstreamCommits').should('have.been.calledOnce');
		cy.get('@integrateUpstreamCommits').should('have.been.calledWith', {
			projectId: PROJECT_ID,
			stackId: mockBackend.stackId,
			seriesName: mockBackend.stackId,
			strategy: undefined
		});
	});

	it('should be able to rename a branch from the context menu', () => {
		const newBranchName = 'new-branch-name';
		// Click on the branch.
		// And then open the context menu.
		cy.getByTestId('branch-header', mockBackend.localOnlyBranchStackId)
			.should('be.visible')
			.click()
			.rightclick();

		// The context menu should be visible
		cy.getByTestId('branch-header-context-menu').should('be.visible');
		cy.getByTestId('branch-header-context-menu-rename').should('be.visible').click();

		// The rename dialog should be visible
		cy.getByTestId('branch-header-rename-modal').should('be.visible');

		// Rename the branch
		cy.get('#new-branch-name-input').should('be.visible').clear().type(newBranchName);
		cy.getByTestId('branch-header-rename-modal-action-button').should('be.visible').click();

		// The branch should be updated in the list header
		cy.getByTestId('branch-header', newBranchName).should('be.visible');

		// The branch name should be visible in the branch view
		cy.getByTestId('branch-view').should('be.visible');
	});

	it('should be able to delete a branch from the context menu', () => {
		// Click on the branch.
		// And then open the context menu.
		cy.getByTestId('branch-header', mockBackend.localOnlyBranchStackId)
			.should('be.visible')
			.click()
			.rightclick();

		// The context menu should be visible
		cy.getByTestId('branch-header-context-menu').should('be.visible');
		cy.getByTestId('branch-header-context-menu-delete').should('be.visible').click();

		// The delete dialog should be visible
		cy.getByTestId('branch-header-delete-modal').should('be.visible');

		// Delete the branch
		cy.getByTestId('branch-header-delete-modal-action-button').should('be.visible').click();

		// The branch should be removed from the list header
		cy.getByTestId('branch-header', mockBackend.localOnlyBranchStackId).should('not.exist');
	});

	it('should be able to add a dependent branch from the context menu', () => {
		const dependentBranchName = 'dependent-branch-name';
		// Click on the branch.
		// And then open the context menu.
		cy.getByTestId('branch-header', mockBackend.localOnlyBranchStackId)
			.should('be.visible')
			.click()
			.rightclick();

		// The context menu should be visible
		cy.getByTestId('branch-header-context-menu').should('be.visible');
		cy.getByTestId('branch-header-context-menu-add-dependent-branch').should('be.visible').click();

		// The add dependent branch dialog should be visible
		cy.getByTestId('branch-header-add-dependent-branch-modal')
			.should('be.visible')
			.within(() => {
				// Add the dependent branch
				cy.get('input[type="text"]').should('be.visible').type(dependentBranchName);
			});

		cy.getByTestId('branch-header-add-dependent-branch-modal-action-button')
			.should('be.visible')
			.click();
	});
});
