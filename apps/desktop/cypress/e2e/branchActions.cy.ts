import { clearCommandMocks, mockCommand } from './support';
import MockBackend from './support/mock/backend';
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
		mockCommand('list_workspace_rules', (params) => mockBackend.listWorkspaceRules(params));
		mockCommand('get_author_info', (params) => mockBackend.getAuthorInfo(params));

		cy.visit('/');

		cy.urlMatches(`/${PROJECT_ID}/workspace`);
	});

	afterEach(() => {
		clearCommandMocks();
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
});

describe('Branch Actions - single branch with uncommitted changes', () => {
	let mockBackend: MockBackend;

	beforeEach(() => {
		mockBackend = new MockBackend();
		mockCommand('stacks', () => mockBackend.getStacks());
		mockCommand('create_virtual_branch', (params) => mockBackend.createBranch(params));
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('update_commit_message', (params) => mockBackend.updateCommitMessage(params));
		mockCommand('changes_in_worktree', (params) => mockBackend.getWorktreeChanges(params));
		mockCommand('tree_change_diffs', (params) => mockBackend.getDiff(params));
		mockCommand('hunk_assignments', (params) => mockBackend.getHunkAssignments(params));
		mockCommand('commit_details', (params) => mockBackend.getCommitChanges(params));
		mockCommand('create_commit_from_worktree_changes', (params) =>
			mockBackend.createCommit(params)
		);
		mockCommand('undo_commit', (params) => mockBackend.undoCommit(params));
		mockCommand('canned_branch_name', () => mockBackend.getCannedBranchName());
		mockCommand('list_workspace_rules', (params) => mockBackend.listWorkspaceRules(params));
		mockCommand('get_author_info', (params) => mockBackend.getAuthorInfo(params));

		cy.visit('/');

		cy.urlMatches(`/${PROJECT_ID}/workspace`);
	});

	it('should be able to create a new branch from the ChromeHeader context menu', () => {
		const newBranchName = 'new-branch-from-workspace';

		// Click the button to commit into new branch
		cy.getByTestId('commit-to-new-branch-button').should('be.visible').click();

		// The commit title should be visible
		cy.getByTestId('commit-drawer-title-input').should('be.visible').should('have.value', '');

		// Cancel the commit
		cy.getByTestId('commit-drawer-cancel-button').should('be.visible').click();

		// Click on the "Create branch" button in the ChromeHeader
		cy.getByTestId('chrome-header-create-branch-button').should('be.visible').click();

		// The create branch dialog should be visible
		cy.getByTestId('create-new-branch-modal').should('be.visible');

		cy.get('#new-branch-name-input').should('be.visible').clear().type(newBranchName);

		cy.getByTestId('confirm-submit').should('be.visible').should('be.enabled').click();

		cy.getByTestId('branch-header', newBranchName).should('be.visible');
	});
});
