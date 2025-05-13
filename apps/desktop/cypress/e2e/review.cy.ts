import { clearCommandMocks, mockCommand } from './support';
import { PROJECT_ID } from './support/mock/projects';
import BranchesWithChanges from './support/scenarios/branchesWithChanges';

describe('Review', () => {
	let mockBackend: BranchesWithChanges;

	beforeEach(() => {
		mockBackend = new BranchesWithChanges();

		mockCommand('stacks', () => mockBackend.getStacks());
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('changes_in_branch', (args) => mockBackend.getBranchChanges(args));
		mockCommand('changes_in_worktree', (params) => mockBackend.getWorktreeChanges(params));
		mockCommand('tree_change_diffs', (params) => mockBackend.getDiff(params));
		mockCommand('hunk_dependencies_for_workspace_changes', (params) =>
			mockBackend.getHunkDependencies(params)
		);
		mockCommand('get_base_branch_data', () => mockBackend.getBaseBranchData());
		mockCommand('get_available_review_templates', () => mockBackend.getAvailableReviewTemplates());
		mockCommand('push_stack', (params) => mockBackend.pushStack(params));
		mockCommand('list_remotes', (params) => mockBackend.listRemotes(params));
		mockCommand('update_branch_pr_number', (params) => mockBackend.updateBranchPrNumber(params));

		cy.visit('/');

		cy.url({ timeout: 3000 }).should('include', `/${PROJECT_ID}/workspace`);
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('should be able to open a GitHub pull request', () => {
		cy.intercept(
			{
				method: 'POST',
				url: 'https://api.github.com/repos/example/repo/pulls'
			},
			{
				statusCode: 201,
				body: {
					number: 42
				}
			}
		).as('createPullRequest');

		cy.intercept(
			{
				method: 'GET',
				url: 'https://api.github.com/repos/example/repo'
			},
			{
				statusCode: 200
			}
		).as('getRepo');

		cy.intercept(
			{
				method: 'GET',
				url: 'https://api.github.com/repos/example/repo/pulls/42'
			},
			{
				statusCode: 200,
				body: {
					number: 42,
					state: 'open'
				}
			}
		).as('getPullRequest');

		cy.intercept(
			{
				method: 'GET',
				url: 'https://api.github.com/repos/example/repo/commits/check-runs'
			},
			{
				statusCode: 200,
				body: {
					total_count: 0,
					check_runs: []
				}
			}
		).as('getChecks');

		const prTitle = 'Test PR Title';
		const prDescription = 'Test PR Description';

		// The branch should be applied. Click it.
		cy.getByTestId('branch-header', mockBackend.stackId).should('be.visible').click();

		// The Create Branch Review button should be visible.
		// Click it.
		cy.getByTestId('branch-drawer-create-review-button').should('be.visible').click();

		// The Review Drawer should be visible.
		cy.getByTestId('review-drawer').should('be.visible');

		// Since this branch has a single commit, the commit message should be pre-filled.
		// Update both.
		cy.getByTestId('review-drawer-title-input')
			.should('be.visible')
			.should('be.enabled')
			.should('have.value', mockBackend.commitTitle)
			.clear()
			.type(prTitle);

		cy.getByTestId('review-drawer-description-input')
			.should('be.visible')
			.should('contain', mockBackend.commitMessage)
			.click()
			.clear()
			.type(prDescription);

		// The Create Review button should be visible.
		// Click it.
		cy.getByTestId('review-drawer-create-button').should('be.visible').should('be.enabled').click();

		// The PR card should be visible.
		cy.getByTestId('stacked-pull-request-card').should('be.visible');
	});
});
