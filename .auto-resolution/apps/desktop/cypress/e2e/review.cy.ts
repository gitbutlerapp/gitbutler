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
		mockCommand('get_base_branch_data', () => mockBackend.getBaseBranchData());
		mockCommand('get_available_review_templates', () => mockBackend.getAvailableReviewTemplates());
		mockCommand('push_stack', (params) => mockBackend.pushStack(params));
		mockCommand('list_remotes', (params) => mockBackend.listRemotes(params));
		mockCommand('update_branch_pr_number', (params) => mockBackend.updateBranchPrNumber(params));
		mockCommand('hunk_assignments', (params) => mockBackend.getHunkAssignments(params));

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
				url: 'https://api.github.com/repos/example/repo/pulls'
			},
			{
				statusCode: 200,
				body: []
			}
		).as('listPullRequests');

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

		cy.visit('/');

		cy.urlMatches(`/${PROJECT_ID}/workspace`);
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('should be able to open a GitHub pull request', () => {
		const prTitle = 'Test PR Title';
		const prDescription = 'Test PR Description';

		// The branch should be applied. Click it.
		cy.getByTestId('branch-header', mockBackend.stackId).should('be.visible').click();

		// The Create Branch Review button should be visible.
		// Click it.
		cy.getByTestId('create-review-button').first().should('be.visible').click();

		// The Review Drawer should be visible.
		cy.getByTestId('review-view').should('be.visible');

		// Since this branch has a single commit, the commit message should be pre-filled.
		// Update both.
		cy.getByTestId('review-view-title-input')
			.should('be.visible')
			.should('be.enabled')
			.should('have.value', mockBackend.getCommitTitle(mockBackend.stackId))
			.clear()
			.type(prTitle);

		cy.getByTestId('review-view-description-input')
			.should('be.visible')
			.should('contain', mockBackend.getCommitMessage(mockBackend.stackId))
			.click()
			.clear()
			.type(prDescription);

		// Cancel the creation of the review.
		cy.getByTestId('review-view-cancel-button').should('be.visible').click();

		// The Review Drawer should not be visible.
		cy.getByTestId('review-view').should('not.exist');

		// Reopen the Review Drawer.
		cy.getByTestId('create-review-button').first().should('be.visible').click();

		// The inputs should be persisted
		cy.getByTestId('review-view-title-input')
			.should('be.visible')
			.should('be.enabled')
			.should('have.value', prTitle);

		cy.getByTestId('review-view-description-input')
			.should('be.visible')
			.should('contain', prDescription);

		// The Create Review button should be visible.
		// Click it.
		cy.getByTestId('review-view-create-button').should('be.visible').should('be.enabled').click();

		// The PR card should be visible.
		cy.getByTestId('stacked-pull-request-card').should('be.visible');
	});

	it('should be able to create multiple pull requests', () => {
		const stacks = mockBackend.getStacks();
		expect(stacks).to.have.length(3);

		for (const stack of stacks) {
			const prTitle = 'Test PR Title' + stack.id;
			const prDescription = 'Test PR Description' + stack.id;

			// The branch should be applied. Click it.
			cy.getByTestId('branch-header', stack.id).scrollIntoView().should('be.visible').click();
			cy.get(`[data-id="${stack.id}"]`).within(() => {
				// The Create Branch Review button should be visible.
				// Click it.
				cy.getByTestId('create-review-button').first().should('be.visible').click();

				// The Review Drawer should be visible.
				cy.getByTestId('review-view').should('be.visible');

				// Since this branch has a single commit, the commit message should be pre-filled.
				// Update both.
				cy.getByTestId('review-view-title-input')
					.should('be.visible')
					.should('be.enabled')
					.should('have.value', mockBackend.getCommitTitle(stack.id))
					.clear()
					.type(prTitle);

				cy.getByTestId('review-view-description-input')
					.should('be.visible')
					.should('contain', mockBackend.getCommitMessage(stack.id))
					.click()
					.clear()
					.type(prDescription);

				// Cancel the creation of the review.
				cy.getByTestId('review-view-cancel-button').should('be.visible').click();

				// The Review Drawer should not be visible.
				cy.getByTestId('review-view').should('not.exist');

				// Reopen the Review Drawer.
				cy.getByTestId('create-review-button').first().should('be.visible').click();

				// The inputs should be persisted
				cy.getByTestId('review-view-title-input')
					.should('be.visible')
					.should('be.enabled')
					.should('have.value', prTitle);

				cy.getByTestId('review-view-description-input')
					.should('be.visible')
					.should('contain', prDescription);

				// The Create Review button should be visible.
				// Click it.
				cy.getByTestId('review-view-create-button')
					.should('be.visible')
					.should('be.enabled')
					.click();

				// The PR card should be visible.
			});
			cy.get(`[data-details="${stack.id}"]`).within(() => {
				cy.getByTestId('stacked-pull-request-card').should('be.visible');
			});
		}
	});

	it('should be able to create multiple pull requests from the publish buttons', () => {
		const stacks = mockBackend.getStacks();
		expect(stacks).to.have.length(3);

		for (const stack of stacks) {
			const prTitle = 'Test PR Title' + stack.id;
			const prDescription = 'Test PR Description' + stack.id;

			// Scroll the publish buttons into view
			cy.get('[data-id="' + stack.id + '"]')
				.should('exist')
				.scrollIntoView()
				.within(() => {
					cy.getByTestId('create-review-button').should('be.visible').click();

					// The Review Drawer should be visible.
					cy.getByTestId('review-view').should('be.visible');

					// Since this branch has a single commit, the commit message should be pre-filled.
					// Update both.
					cy.getByTestId('review-view-title-input')
						.should('be.visible')
						.should('be.enabled')
						.should('have.value', mockBackend.getCommitTitle(stack.id))
						.clear()
						.type(prTitle);

					cy.getByTestId('review-view-description-input')
						.should('be.visible')
						.should('contain', mockBackend.getCommitMessage(stack.id))
						.click()
						.clear()
						.type(prDescription);

					// Cancel the creation of the review.
					cy.getByTestId('review-view-cancel-button').should('be.visible').click();

					// The Review Drawer should not be visible.
					cy.getByTestId('review-view').should('not.exist');

					// Reopen the Review Drawer.
					cy.getByTestId('create-review-button')
						.first()
						.scrollIntoView()
						.should('be.visible')
						.click();

					// The inputs should be persisted
					cy.getByTestId('review-view-title-input')
						.should('be.visible')
						.should('be.enabled')
						.should('have.value', prTitle);

					cy.getByTestId('review-view-description-input')
						.should('be.visible')
						.should('contain', prDescription);

					// The Create Review button should be visible.
					// Click it.
					cy.getByTestId('review-view-create-button')
						.should('be.visible')
						.should('be.enabled')
						.click();

					// Click branch header to reveal pull request card.
					cy.getByTestId('branch-header', stack.id).should('be.visible').click();
				});
			cy.get(`[data-details="${stack.id}"]`).within(() => {
				// The PR card should be visible.
				cy.getByTestId('stacked-pull-request-card').should('be.visible');
			});
		}
	});
});
