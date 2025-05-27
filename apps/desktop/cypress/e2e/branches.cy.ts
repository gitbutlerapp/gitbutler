import { clearCommandMocks, mockCommand } from './support';
import { PROJECT_ID } from './support/mock/projects';
import UnappliedBranchesAndTargetCommits from './support/scenarios/unappliedBranchesAndTargetCommits';

describe('Branches', () => {
	let mockBackend: UnappliedBranchesAndTargetCommits;

	beforeEach(() => {
		mockBackend = new UnappliedBranchesAndTargetCommits();
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('update_commit_message', (params) => mockBackend.updateCommitMessage(params));
		mockCommand('changes_in_worktree', (params) => mockBackend.getWorktreeChanges(params));
		mockCommand('tree_change_diffs', (params) => mockBackend.getDiff(params));
		mockCommand('commit_details', (params) => mockBackend.getCommitChanges(params));
		mockCommand('create_commit_from_worktree_changes', (params) =>
			mockBackend.createCommit(params)
		);
		mockCommand('undo_commit', (params) => mockBackend.undoCommit(params));
		mockCommand('list_branches', (params) => mockBackend.listBranches(params));
		mockCommand('branch_details', (params) => mockBackend.getBranchDetails(params));
		mockCommand('changes_in_branch', (args) => mockBackend.getBranchChanges(args));
		mockCommand('target_commits', (args) => mockBackend.getBaseBranchCommits(args));
		mockCommand('list_remotes', (params) => mockBackend.listRemotes(params));
		mockCommand('create_virtual_branch_from_branch', (args) =>
			mockBackend.createVirtualBranchFromBranch(args)
		);
		mockCommand('delete_local_branch', (params) => mockBackend.deleteLocalBranch(params));
		mockCommand('get_branch_listing_details', () => []);
		mockCommand('add_remote', (params) => mockBackend.addRemote(params));

		cy.intercept(
			{
				method: 'GET',
				url: 'https://api.github.com/repos/example/repo/pulls'
			},
			{
				statusCode: 200,
				body: mockBackend.getMockPRListings()
			}
		).as('listPullRequests');

		cy.intercept(
			{
				method: 'GET',
				url: 'https://api.github.com/repos/example/repo/pulls/42'
			},
			{
				statusCode: 200,
				body: mockBackend.getMockPr()
			}
		).as('getPullRequest');

		cy.visit('/');

		cy.url({ timeout: 3000 }).should('include', `/${PROJECT_ID}/workspace`);

		// Click on the branches button
		cy.getByTestId('navigation-branches-button').should('be.visible').should('be.enabled').click();

		// Be able to see the branches page
		cy.url({ timeout: 3000 }).should('include', `/${PROJECT_ID}/branches`);
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('should navigate to the branches page when clicking the branches button', () => {
		// The target branch should be automatically selected
		cy.getByTestId('target-commit-list-header')
			.should('be.visible')
			.should('contain', mockBackend.getBaseBranchName());

		// The branch drawer should be visible
		cy.getByTestId('unapplied-branch-drawer')
			.should('be.visible')
			.should('contain', mockBackend.getBaseBranchName());

		// The branch action buttons should not be visible
		cy.getByTestId('branches-view-apply-branch-button').should('not.exist');
		cy.getByTestId('branches-view-delete-local-branch-button').should('not.exist');
	});

	it('should navigate to the workspace after applying a branch', () => {
		// Click on the first branch
		cy.getByTestId('branch-list-card', mockBackend.branchListing.name)
			.first()
			.should('be.visible')
			.click();

		// The branch should be displayed
		cy.getByTestId('branch-header')
			.should('be.visible')
			.should('contain', mockBackend.branchListing.name);

		// The branch drawer should be visible
		cy.getByTestId('unapplied-branch-drawer')
			.should('be.visible')
			.should('contain', mockBackend.branchListing.name);

		// The branch action buttons should be visible
		cy.getByTestId('branches-view-apply-branch-button').should('be.visible').should('be.enabled');
		cy.getByTestId('branches-view-delete-local-branch-button')
			.should('be.visible')
			.should('be.enabled');

		// Click on the apply branch button
		cy.getByTestId('branches-view-apply-branch-button').click();

		// The workspace should be displayed
		cy.url({ timeout: 3000 }).should('include', `/${PROJECT_ID}/workspace`);
	});

	it('should be able to delete a local branch', () => {
		// Click on the first branch
		cy.getByTestId('branch-list-card', mockBackend.branchListing.name)
			.first()
			.should('be.visible')
			.click();

		// The branch should be displayed
		cy.getByTestId('branch-header')
			.should('be.visible')
			.should('contain', mockBackend.branchListing.name);

		// The branch drawer should be visible
		cy.getByTestId('unapplied-branch-drawer')
			.should('be.visible')
			.should('contain', mockBackend.branchListing.name);

		// The branch action buttons should be visible
		cy.getByTestId('branches-view-apply-branch-button').should('be.visible').should('be.enabled');
		cy.getByTestId('branches-view-delete-local-branch-button')
			.should('be.visible')
			.should('be.enabled');

		// Click on the delete branch button
		cy.getByTestId('branches-view-delete-local-branch-button').click();

		// The delete branch confirmation modal should be displayed
		cy.getByTestId('delete-local-branch-confirmation-modal')
			.should('be.visible')
			.should('contain', mockBackend.branchListing.name);

		// Click on the cancel button
		cy.getByTestId('delete-local-branch-confirmation-modal-cancel').click();

		// The delete branch confirmation modal should be closed
		cy.getByTestId('delete-local-branch-confirmation-modal').should('not.exist');

		// The branch drawer should be visible
		cy.getByTestId('unapplied-branch-drawer')
			.should('be.visible')
			.should('contain', mockBackend.branchListing.name);

		// Click on the delete branch button
		cy.getByTestId('branches-view-delete-local-branch-button').click();

		// The delete branch confirmation modal should be displayed
		cy.getByTestId('delete-local-branch-confirmation-modal')
			.should('be.visible')
			.should('contain', mockBackend.branchListing.name);

		// Click on the delete button
		cy.getByTestId('delete-local-branch-confirmation-modal-delete').click();

		// The delete branch confirmation modal should be closed
		cy.getByTestId('delete-local-branch-confirmation-modal').should('not.exist');

		// The branch drawer should be visible but should show the base branch name
		cy.getByTestId('unapplied-branch-drawer')
			.should('be.visible')
			.should('contain', mockBackend.getBaseBranchName());
	});

	it('should be able to apply a branch from a fork', () => {
		const forkRemoteName = 'fork-remote';
		// Click on the PR branch card
		cy.getByTestId('pr-list-card')
			.should('be.visible')
			.should('have.length', mockBackend.getMockPRListings().length)
			.click();

		// The PR branch drawe should be visible
		cy.getByTestId('pr-branch-drawer')
			.should('be.visible')
			.should('contain', mockBackend.getMockPr().head.ref)
			.should('contain', mockBackend.getMockPr().title)
			.should('contain', mockBackend.getMockPr().body);

		// The button to apply the branch from a fork should be visible
		cy.getByTestId('branches-view-apply-from-fork-button')
			.should('be.visible')
			.should('be.enabled')
			.click();

		// The create remote branch modal should be visible
		cy.getByTestId('branches-view-create-remote-modal')
			.should('be.visible')
			.within(() => {
				cy.get('input[type="text"]').type(forkRemoteName);
			});

		// Click on the action button to create the remote branch
		cy.getByTestId('branches-view-create-remote-modal-action-button')
			.should('be.visible')
			.should('be.enabled')
			.click();

		// Should have navigation to the workspace
		cy.url({ timeout: 3000 }).should('include', `/${PROJECT_ID}/workspace`);
	});
});
