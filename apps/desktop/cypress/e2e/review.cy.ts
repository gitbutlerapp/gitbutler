import { clearCommandMocks, mockCommand } from './support';
import { PROJECT_ID } from './support/mock/projects';
import BranchesWithChanges from './support/scenarios/branchesWithChanges';
import StackBranchesWithCommits from './support/scenarios/stackBrancheshWithCommits';
import type { ChecksResult } from '$lib/forge/github/types';

describe('Review', () => {
	let mockBackend: BranchesWithChanges;

	beforeEach(() => {
		mockBackend = new BranchesWithChanges();

		mockCommand('stacks', () => mockBackend.getStacks());
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('changes_in_branch', (args) => mockBackend.getBranchChanges(args));
		mockCommand('changes_in_worktree', (params) => mockBackend.getWorktreeChanges(params));
		mockCommand('tree_change_diffs', (params) => mockBackend.getDiff(params));
		mockCommand('get_base_branch_data', () => mockBackend.getBaseBranchData(undefined));
		mockCommand('pr_templates', () => mockBackend.getAvailableReviewTemplates());
		mockCommand('push_stack', (params) => mockBackend.pushStack(params));
		mockCommand('list_remotes', (params) => mockBackend.listRemotes(params));
		mockCommand('update_branch_pr_number', (params) => mockBackend.updateBranchPrNumber(params));
		mockCommand('hunk_assignments', (params) => mockBackend.getHunkAssignments(params));
		mockCommand('pr_template', (args) => mockBackend.getTemplateContent(args));
		mockCommand('list_workspace_rules', (params) => mockBackend.listWorkspaceRules(params));
		mockCommand('get_author_info', (params) => mockBackend.getAuthorInfo(params));

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

		let initialFetchPull42 = undefined;

		cy.intercept(
			{
				method: 'GET',
				url: 'https://api.github.com/repos/example/repo/pulls/42'
			},
			(req) => {
				initialFetchPull42 ??= new Date().toISOString();
				req.reply({
					statusCode: 200,
					body: {
						number: 42,
						state: 'open',
						updated_at: initialFetchPull42
					}
				});
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
		cy.getByTestId('create-review-box').should('be.visible');

		// Since this branch has a single commit, the commit message should be pre-filled.
		// Update both.
		cy.getByTestId('create-review-box-title-input')
			.should('be.visible')
			.should('be.enabled')
			.should('have.value', mockBackend.getCommitTitle(mockBackend.stackId))
			.clear()
			.type(prTitle);

		cy.getByTestId('create-review-box-description-input')
			.should('be.visible')
			.should('contain', mockBackend.getCommitMessage(mockBackend.stackId))
			.click()
			.clear()
			.type(prDescription);

		// Cancel the creation of the review.
		cy.getByTestId('create-review-box-cancel-button').should('be.visible').click();

		// The Review Drawer should not be visible.
		cy.getByTestId('create-review-box').should('not.exist');

		// Reopen the Review Drawer.
		cy.getByTestId('create-review-button').first().should('be.visible').click();

		// The inputs should be persisted
		cy.getByTestId('create-review-box-title-input')
			.should('be.visible')
			.should('be.enabled')
			.should('have.value', prTitle);

		cy.getByTestId('create-review-box-description-input')
			.should('be.visible')
			.should('contain', prDescription);

		// The Create Review button should be visible.
		// Click it.
		cy.getByTestId('create-review-box-create-button')
			.should('be.visible')
			.should('be.enabled')
			.click();

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
				cy.getByTestId('create-review-box').should('be.visible');

				// Since this branch has a single commit, the commit message should be pre-filled.
				// Update both.
				cy.getByTestId('create-review-box-title-input')
					.should('be.visible')
					.should('be.enabled')
					.should('have.value', mockBackend.getCommitTitle(stack.id!))
					.clear()
					.type(prTitle);

				cy.getByTestId('create-review-box-description-input')
					.should('be.visible')
					.should('contain', mockBackend.getCommitMessage(stack.id!))
					.click()
					.clear()
					.type(prDescription);

				// Cancel the creation of the review.
				cy.getByTestId('create-review-box-cancel-button').should('be.visible').click();

				// The Review Drawer should not be visible.
				cy.getByTestId('create-review-box').should('not.exist');

				// Reopen the Review Drawer.
				cy.getByTestId('create-review-button').first().should('be.visible').click();

				// The inputs should be persisted
				cy.getByTestId('create-review-box-title-input')
					.should('be.visible')
					.should('be.enabled')
					.should('have.value', prTitle);

				cy.getByTestId('create-review-box-description-input')
					.should('be.visible')
					.should('contain', prDescription);

				// The Create Review button should be visible.
				// Click it.
				cy.getByTestId('create-review-box-create-button')
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
					cy.getByTestId('create-review-box').should('be.visible');

					// Since this branch has a single commit, the commit message should be pre-filled.
					// Update both.
					cy.getByTestId('create-review-box-title-input')
						.should('be.visible')
						.should('be.enabled')
						.should('have.value', mockBackend.getCommitTitle(stack.id!))
						.clear()
						.type(prTitle);

					cy.getByTestId('create-review-box-description-input')
						.should('be.visible')
						.should('contain', mockBackend.getCommitMessage(stack.id!))
						.click()
						.clear()
						.type(prDescription);

					// Cancel the creation of the review.
					cy.getByTestId('create-review-box-cancel-button').should('be.visible').click();

					// The Review Drawer should not be visible.
					cy.getByTestId('create-review-box').should('not.exist');

					// Reopen the Review Drawer.
					cy.getByTestId('create-review-button')
						.first()
						.scrollIntoView()
						.should('be.visible')
						.click();

					// The inputs should be persisted
					cy.getByTestId('create-review-box-title-input')
						.should('be.visible')
						.should('be.enabled')
						.should('have.value', prTitle);

					cy.getByTestId('create-review-box-description-input')
						.should('be.visible')
						.should('contain', prDescription);

					// The Create Review button should be visible.
					// Click it.
					cy.getByTestId('create-review-box-create-button')
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

	it('should be able to create multiple pull requests with templates', () => {
		const stacks = mockBackend.getStacks();
		expect(stacks).to.have.length(3);
		let enabledTemplates = false;

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
					cy.getByTestId('create-review-box').should('be.visible');

					// The template toggle should be visible and enabled.
					cy.getByTestId('create-review-box-template-toggle')
						.should('be.visible')
						.should('be.enabled');

					// If the template toggle is not enabled, enable it.
					if (!enabledTemplates) {
						// Since this branch has a single commit, the commit message should be pre-filled.
						// Update both.
						cy.getByTestId('create-review-box-title-input')
							.should('be.visible')
							.should('be.enabled')
							.should('have.value', mockBackend.getCommitTitle(stack.id!));

						cy.getByTestId('create-review-box-template-toggle').click();
						enabledTemplates = true;
					}
					// Since this branch has a single commit, the commit message should be pre-filled.
					// Update both.
					cy.getByTestId('create-review-box-title-input')
						.should('be.visible')
						.should('be.enabled')
						.should('have.value', mockBackend.getCommitTitle(stack.id!))
						.clear()
						.type(prTitle);

					for (const line of mockBackend.prTemplateContent.split('\n')) {
						cy.getByTestId('create-review-box-description-input')
							.should('be.visible')
							.should('contain', line);
					}

					cy.getByTestId('create-review-box-description-input')
						.should('be.visible')
						.click()
						.clear()
						.type(prDescription);

					// Cancel the creation of the review.
					cy.getByTestId('create-review-box-cancel-button').should('be.visible').click();

					// The Review Drawer should not be visible.
					cy.getByTestId('create-review-box').should('not.exist');

					// Reopen the Review Drawer.
					cy.getByTestId('create-review-button')
						.first()
						.scrollIntoView()
						.should('be.visible')
						.click();

					// The inputs should be persisted
					cy.getByTestId('create-review-box-title-input')
						.should('be.visible')
						.should('be.enabled')
						.should('have.value', prTitle);

					cy.getByTestId('create-review-box-description-input')
						.should('be.visible')
						.should('contain', prDescription);

					// The Create Review button should be visible.
					// Click it.
					cy.getByTestId('create-review-box-create-button')
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

describe('Review - stacked branches', () => {
	let mockBackend: StackBranchesWithCommits;

	beforeEach(() => {
		mockBackend = new StackBranchesWithCommits();

		mockCommand('stacks', () => mockBackend.getStacks());
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('changes_in_branch', (args) => mockBackend.getBranchChanges(args));
		mockCommand('changes_in_worktree', (params) => mockBackend.getWorktreeChanges(params));
		mockCommand('tree_change_diffs', (params) => mockBackend.getDiff(params));
		mockCommand('get_base_branch_data', () => mockBackend.getBaseBranchData(undefined));
		mockCommand('pr_templates', () => mockBackend.getAvailableReviewTemplates());
		mockCommand('push_stack', (params) => mockBackend.pushStack(params));
		mockCommand('list_remotes', (params) => mockBackend.listRemotes(params));
		mockCommand('update_branch_pr_number', (params) => mockBackend.updateBranchPrNumber(params));
		mockCommand('hunk_assignments', (params) => mockBackend.getHunkAssignments(params));
		mockCommand('pr_template', (args) => mockBackend.getTemplateContent(args));
		mockCommand('update_commit_message', (params) => mockBackend.updateCommitMessage(params));
		mockCommand('list_workspace_rules', (params) => mockBackend.listWorkspaceRules(params));
		mockCommand('get_author_info', (params) => mockBackend.getAuthorInfo(params));

		cy.intercept(
			{
				method: 'POST',
				url: 'https://api.github.com/repos/example/repo/pulls'
			},
			(req) => {
				const prNumber = req.body.head === mockBackend.bottomBranchName ? 42 : 43;
				req.reply({
					statusCode: 201,
					body: {
						number: prNumber
					}
				});
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

		let initialFetchPull42 = undefined;

		cy.intercept(
			{
				method: 'GET',
				url: 'https://api.github.com/repos/example/repo/pulls/42'
			},
			(req) => {
				initialFetchPull42 ??= new Date().toISOString();
				req.reply({
					statusCode: 200,
					body: {
						number: 42,
						state: 'open',
						head: {
							ref: mockBackend.bottomBranchName
						},
						updated_at: initialFetchPull42
					}
				});
			}
		).as('getPullRequest42');

		let initialFetchPull43 = undefined;

		cy.intercept(
			{
				method: 'GET',
				url: 'https://api.github.com/repos/example/repo/pulls/43'
			},
			(req) => {
				initialFetchPull43 ??= new Date().toISOString();
				req.reply({
					statusCode: 200,
					body: {
						number: 43,
						state: 'open',
						head: {
							ref: mockBackend.topBranchName
						},
						updated_at: initialFetchPull43
					}
				});
			}
		).as('getPullRequest43');

		cy.intercept(
			{
				method: 'GET',
				url: `https://api.github.com/repos/example/repo/commits/${mockBackend.topBranchName}/check-runs`
			},
			{
				statusCode: 200,
				body: {
					total_count: 0,
					check_runs: []
				}
			}
		).as('getChecksTop');

		cy.intercept(
			{
				method: 'GET',
				url: `https://api.github.com/repos/example/repo/commits/${mockBackend.bottomBranchName}/check-runs`
			},
			{
				statusCode: 200,
				body: {
					total_count: 0,
					check_runs: []
				}
			}
		).as('getChecksBottom');

		cy.intercept(
			{
				method: 'PATCH',
				url: 'https://api.github.com/repos/example/repo/pulls/42'
			},
			{
				statusCode: 200,
				body: {
					number: 42,
					state: 'open'
				}
			}
		).as('updatePullRequest42');

		cy.intercept(
			{
				method: 'PATCH',
				url: 'https://api.github.com/repos/example/repo/pulls/43'
			},
			{
				statusCode: 200,
				body: {
					number: 43,
					state: 'open'
				}
			}
		).as('updatePullRequest43');

		cy.visit('/');

		cy.urlMatches(`/${PROJECT_ID}/workspace`);
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('Should be able to create a pull request from a stacked branch - bottom up', () => {
		const prTitle1 = 'Test PR Title';
		const prDescription1 = 'Test PR Description';

		const prTitle2 = 'Test PR Title 2';
		const prDescription2 = 'Test PR Description 2';

		// The branch should be applied. Click it.
		cy.getByTestId('branch-header', mockBackend.bottomBranchName).should('be.visible').click();

		// Both 'create review' buttons should be visible.
		cy.getByTestId('create-review-button').should('have.length', 2);

		// Click the bottom branch 'create review' button.
		cy.getByDataValue('series-name', mockBackend.bottomBranchName).within(() => {
			cy.getByTestId('create-review-button')
				.should('have.length', 1)
				.should('be.visible')
				.should('be.enabled')
				.click();
		});

		// The Review Drawer should be visible.
		cy.getByTestId('create-review-box').should('be.visible').should('have.length', 1);

		// Since this branch has a single commit, the commit message should be pre-filled.
		// Update both.
		cy.getByTestId('create-review-box-title-input')
			.should('be.visible')
			.should('be.enabled')
			.should('have.value', mockBackend.getCommitTitle(mockBackend.bottomBranchName))
			.clear()
			.type(prTitle1);

		cy.getByTestId('create-review-box-description-input')
			.should('be.visible')
			.should('contain', mockBackend.getCommitMessage(mockBackend.bottomBranchName))
			.click()
			.clear()
			.type(prDescription1);

		// Cancel the creation of the review.
		cy.getByTestId('create-review-box-cancel-button').should('be.visible').click();

		// The Review Drawer should not be visible.
		cy.getByTestId('create-review-box').should('not.exist');

		// Reopen the Review Drawer.
		// Click the bottom branch 'create review' button.
		cy.getByDataValue('series-name', mockBackend.bottomBranchName).within(() => {
			cy.getByTestId('create-review-button')
				.should('have.length', 1)
				.should('be.visible')
				.should('be.enabled')
				.click();
		});

		// The inputs should be persisted
		cy.getByTestId('create-review-box-title-input')
			.should('be.visible')
			.should('be.enabled')
			.should('have.value', prTitle1);

		cy.getByTestId('create-review-box-description-input')
			.should('be.visible')
			.should('contain', prDescription1);

		// The Create Review button should be visible.
		// Click it.
		cy.getByTestId('create-review-box-create-button')
			.should('be.visible')
			.should('be.enabled')
			.click();

		cy.wait('@createPullRequest').its('request.body').should('deep.equal', {
			head: mockBackend.bottomBranchName,
			base: 'main',
			title: prTitle1,
			body: prDescription1,
			draft: false
		});

		// The PR card should be visible.
		cy.getByTestId('stacked-pull-request-card').should('be.visible');

		// Open the top branch.
		cy.getByTestId('branch-header', mockBackend.topBranchName).should('be.visible').click();

		// The PR card should not be visible for the top branch.
		cy.getByTestId('stacked-pull-request-card').should('not.exist');

		// Now, open a review for the top branch.
		cy.getByDataValue('series-name', mockBackend.topBranchName).within(() => {
			cy.getByTestId('create-review-button')
				.should('have.length', 1)
				.should('be.visible')
				.should('be.enabled')
				.click();
		});

		// The Review Drawer should be visible.
		cy.getByTestId('create-review-box').should('be.visible').should('have.length', 1);

		// Since this branch has a single commit, the commit message should be pre-filled.
		// Update both.
		cy.getByTestId('create-review-box-title-input')
			.should('be.visible')
			.should('be.enabled')
			.should('have.value', mockBackend.getCommitTitle(mockBackend.topBranchName))
			.clear()
			.type(prTitle2);

		cy.getByTestId('create-review-box-description-input')
			.should('be.visible')
			.should('contain', mockBackend.getCommitMessage(mockBackend.topBranchName))
			.click()
			.clear()
			.type(prDescription2);

		// Cancel the creation of the review.
		cy.getByTestId('create-review-box-cancel-button').should('be.visible').click();

		// The Review Drawer should not be visible.
		cy.getByTestId('create-review-box').should('not.exist');

		// Reopen the Review Drawer.
		cy.getByDataValue('series-name', mockBackend.topBranchName).within(() => {
			cy.getByTestId('create-review-button')
				.should('have.length', 1)
				.should('be.visible')
				.should('be.enabled')
				.click();
		});

		// The inputs should be persisted
		cy.getByTestId('create-review-box-title-input')
			.should('be.visible')
			.should('be.enabled')
			.should('have.value', prTitle2);
		cy.getByTestId('create-review-box-description-input')
			.should('be.visible')
			.should('contain', prDescription2);

		// The Create Review button should be visible.
		// Click it.
		cy.getByTestId('create-review-box-create-button')
			.should('be.visible')
			.should('be.enabled')
			.click();

		cy.wait('@createPullRequest').its('request.body').should('deep.equal', {
			head: mockBackend.topBranchName,
			base: mockBackend.bottomBranchName,
			title: prTitle2,
			body: prDescription2,
			draft: false
		});

		cy.wait('@getChecksTop');
		cy.wait('@getChecksBottom');

		cy.wait(10 * 1000).then(() => {
			cy.get('@getChecksTop.all').should('have.length', 3);
			cy.get('@getChecksBottom.all').should('have.length', 3);
		}); // Wait for the checks to be fetched
	});

	type CheckRun = Partial<ChecksResult['check_runs'][number]>;
	type CustomChecksData = {
		total_count: number;
		check_runs: CheckRun[];
	};

	it('Should be able to create a pull request and listen for CI checks', () => {
		const data: CustomChecksData = {
			total_count: 1,
			check_runs: [
				{
					id: 1,
					started_at: new Date(Date.now() - 10000).toISOString(),
					conclusion: null,
					completed_at: null,
					head_sha: 'abc123',
					name: 'CI Check 1',
					status: 'in_progress'
				}
			]
		};

		const finishedData: CustomChecksData = {
			total_count: 1,
			check_runs: [
				{
					...data.check_runs[0],
					status: 'completed',
					conclusion: 'success',
					completed_at: new Date().toISOString()
				}
			]
		};

		let requestCount = 0;

		cy.intercept(
			{
				method: 'GET',
				url: `https://api.github.com/repos/example/repo/commits/${mockBackend.topBranchName}/check-runs`
			},
			(req) => {
				requestCount++;
				if (requestCount > 2) {
					req.reply({
						statusCode: 200,
						body: finishedData
					});
					return;
				}

				req.reply({
					statusCode: 200,
					body: data
				});
			}
		).as('getChecksWithActualChecks');

		const prTitle = 'Test PR Title';
		const prDescription = 'Test PR Description';

		// Open the top branch.
		cy.getByTestId('branch-header', mockBackend.topBranchName).should('be.visible').click();

		// The PR card should not be visible for the top branch.
		cy.getByTestId('stacked-pull-request-card').should('not.exist');

		// Now, open a review for the top branch.
		cy.getByDataValue('series-name', mockBackend.topBranchName).within(() => {
			cy.getByTestId('create-review-button')
				.should('have.length', 1)
				.should('be.visible')
				.should('be.enabled')
				.click();
		});

		// The Review Drawer should be visible.
		cy.getByTestId('create-review-box').should('be.visible').should('have.length', 1);

		// Since this branch has a single commit, the commit message should be pre-filled.
		// Update both.
		cy.getByTestId('create-review-box-title-input')
			.should('be.visible')
			.should('be.enabled')
			.should('have.value', mockBackend.getCommitTitle(mockBackend.topBranchName))
			.clear()
			.type(prTitle);

		cy.getByTestId('create-review-box-description-input')
			.should('be.visible')
			.should('contain', mockBackend.getCommitMessage(mockBackend.topBranchName))
			.click()
			.clear()
			.type(prDescription);

		// The Create Review button should be visible.
		// Click it.
		cy.getByTestId('create-review-box-create-button')
			.should('be.visible')
			.should('be.enabled')
			.click();

		// The PR card should be visible.
		cy.getByTestId('stacked-pull-request-card').should('be.visible');

		cy.getByTestId('stacked-pull-request-card').within(() => {
			cy.getByTestId('pr-status-badge').should('be.visible');
			cy.getByDataValue('pr-status', 'open').should('be.visible');
		});

		cy.getByTestId('branch-card', mockBackend.topBranchName)
			.should('be.visible')
			.within(() => {
				cy.getByTestId('pr-checks-badge').should('be.visible');
			});

		cy.wait(
			['@getChecksWithActualChecks', '@getChecksWithActualChecks', '@getChecksWithActualChecks'],
			{ timeout: 11000 }
		).spread((first, second, third) => {
			expect(first.response.body).to.deep.equal(data);
			expect(second.response.body).to.deep.equal(data);
			expect(third.response.body).to.deep.equal(finishedData);
		});

		cy.getByTestId('stacked-pull-request-card').within(() => {
			cy.getByTestId('pr-status-badge').should('be.visible');
			cy.getByDataValue('pr-status', 'open').should('be.visible');
		});

		cy.getByTestId('branch-card', mockBackend.topBranchName)
			.should('be.visible')
			.within(() => {
				cy.getByTestId('pr-checks-badge').should('be.visible');
			});

		cy.wait(10 * 1000).then(() => {
			cy.get('@getChecksWithActualChecks.all').should('have.length', 3);
			cy.get('@getChecksBottom.all').should('have.length', 0);
		}); // Wait for the checks to be fetched
	});

	it('Should fail fast when checking for multiple checks', () => {
		const data: CustomChecksData = {
			total_count: 2,
			check_runs: [
				{
					id: 1,
					started_at: new Date(Date.now() - 10000).toISOString(),
					conclusion: null,
					completed_at: null,
					head_sha: 'abc123',
					name: 'CI Check 1',
					status: 'in_progress'
				},
				{
					id: 2,
					started_at: new Date(Date.now() - 10000).toISOString(),
					conclusion: null,
					completed_at: null,
					head_sha: 'abc123',
					name: 'CI Check 2',
					status: 'in_progress'
				}
			]
		};

		const oneCheckFailed: CustomChecksData = {
			total_count: 1,
			check_runs: [
				{
					...data.check_runs[0]
				},
				{
					...data.check_runs[1],
					status: 'completed',
					conclusion: 'failure',
					completed_at: new Date().toISOString()
				}
			]
		};

		let requestCount = 0;

		cy.intercept(
			{
				method: 'GET',
				url: `https://api.github.com/repos/example/repo/commits/${mockBackend.topBranchName}/check-runs`
			},
			(req) => {
				requestCount++;
				if (requestCount > 2) {
					req.reply({
						statusCode: 200,
						body: oneCheckFailed
					});
					return;
				}

				req.reply({
					statusCode: 200,
					body: data
				});
			}
		).as('getChecksWithActualChecks');

		const prTitle = 'Test PR Title';
		const prDescription = 'Test PR Description';

		// Open the top branch.
		cy.getByTestId('branch-header', mockBackend.topBranchName).should('be.visible').click();

		// The PR card should not be visible for the top branch.
		cy.getByTestId('stacked-pull-request-card').should('not.exist');

		// Now, open a review for the top branch.
		cy.getByDataValue('series-name', mockBackend.topBranchName).within(() => {
			cy.getByTestId('create-review-button')
				.should('have.length', 1)
				.should('be.visible')
				.should('be.enabled')
				.click();
		});

		// The Review Drawer should be visible.
		cy.getByTestId('create-review-box').should('be.visible').should('have.length', 1);

		// Since this branch has a single commit, the commit message should be pre-filled.
		// Update both.
		cy.getByTestId('create-review-box-title-input')
			.should('be.visible')
			.should('be.enabled')
			.should('have.value', mockBackend.getCommitTitle(mockBackend.topBranchName))
			.clear()
			.type(prTitle);

		cy.getByTestId('create-review-box-description-input')
			.should('be.visible')
			.should('contain', mockBackend.getCommitMessage(mockBackend.topBranchName))
			.click()
			.clear()
			.type(prDescription);

		// The Create Review button should be visible.
		// Click it.
		cy.getByTestId('create-review-box-create-button')
			.should('be.visible')
			.should('be.enabled')
			.click();

		// The PR card should be visible.
		cy.getByTestId('stacked-pull-request-card').should('be.visible');

		cy.getByTestId('stacked-pull-request-card').within(() => {
			cy.getByTestId('pr-status-badge').should('be.visible');
			cy.getByDataValue('pr-status', 'open').should('be.visible');
		});

		cy.getByTestId('branch-card', mockBackend.topBranchName)
			.should('be.visible')
			.within(() => {
				cy.getByTestId('pr-checks-badge').should('be.visible');
			});

		cy.wait(
			['@getChecksWithActualChecks', '@getChecksWithActualChecks', '@getChecksWithActualChecks'],
			{ timeout: 11000 }
		)
			.spread((first, second, third) => {
				expect(first.response.body).to.deep.equal(data);
				expect(second.response.body).to.deep.equal(data);
				expect(third.response.body).to.deep.equal(oneCheckFailed);
			})
			.then(() => {
				cy.getByDataValue('pr-text', 'Failed').should('be.visible');
			});

		cy.getByTestId('stacked-pull-request-card').within(() => {
			cy.getByTestId('pr-status-badge').should('be.visible');
			cy.getByDataValue('pr-status', 'open').should('be.visible');
		});

		cy.wait(10 * 1000).then(() => {
			cy.get('@getChecksWithActualChecks.all').should('have.length', 3);
			cy.get('@getChecksBottom.all').should('have.length', 0);
		}); // Wait for the checks to be fetched
	});

	it('Should restart polling on force push', () => {
		const data: CustomChecksData = {
			total_count: 2,
			check_runs: [
				{
					id: 1,
					started_at: new Date(Date.now() - 10000).toISOString(),
					conclusion: null,
					completed_at: null,
					head_sha: 'abc123',
					name: 'CI Check 1',
					status: 'in_progress'
				},
				{
					id: 2,
					started_at: new Date(Date.now() - 10000).toISOString(),
					head_sha: 'abc123',
					name: 'CI Check 2',
					status: 'completed',
					conclusion: 'failure',
					completed_at: new Date().toISOString()
				}
			]
		};

		cy.intercept(
			{
				method: 'GET',
				url: `https://api.github.com/repos/example/repo/commits/${mockBackend.topBranchName}/check-runs`
			},
			(req) => {
				req.reply({
					statusCode: 200,
					body: data
				});
			}
		).as('getChecksWithActualChecks');

		const prTitle = 'Test PR Title';
		const prDescription = 'Test PR Description';

		// Open the top branch.
		cy.getByTestId('branch-header', mockBackend.topBranchName).should('be.visible').click();

		// The PR card should not be visible for the top branch.
		cy.getByTestId('stacked-pull-request-card').should('not.exist');

		// Now, open a review for the top branch.
		cy.getByDataValue('series-name', mockBackend.topBranchName).within(() => {
			cy.getByTestId('create-review-button')
				.should('have.length', 1)
				.should('be.visible')
				.should('be.enabled')
				.click();
		});

		// The Review Drawer should be visible.
		cy.getByTestId('create-review-box').should('be.visible').should('have.length', 1);

		// Since this branch has a single commit, the commit message should be pre-filled.
		// Update both.
		cy.getByTestId('create-review-box-title-input')
			.should('be.visible')
			.should('be.enabled')
			.clear()
			.type(prTitle);

		cy.getByTestId('create-review-box-description-input')
			.should('be.visible')
			.click()
			.clear()
			.type(prDescription);

		// The Create Review button should be visible.
		// Click it.
		cy.getByTestId('create-review-box-create-button')
			.should('be.visible')
			.should('be.enabled')
			.click();

		// There should have been exactly 2 requests to the checks endpoint
		cy.wait(['@getChecksWithActualChecks', '@getChecksWithActualChecks'], { timeout: 11000 })
			.spread((first, second) => {
				expect(first.response.body).to.deep.equal(data);
				expect(second.response.body).to.deep.equal(data);
			})
			.then(() => {
				cy.getByDataValue('pr-text', 'Failed').should('be.visible');
			});

		cy.getByTestId('stacked-pull-request-card').within(() => {
			cy.getByTestId('pr-status-badge').should('be.visible');
			cy.getByDataValue('pr-status', 'open').should('be.visible');
		});

		// Reword the commit and force push
		cy.getByTestId('commit-row').first().click();

		// Click on the kebab menu to access edit message
		cy.getByTestId('commit-drawer').within(() => {
			cy.getByTestId('kebab-menu-btn').click();
		});

		// Click on the edit message button in the context menu
		cy.getByTestId('commit-row-context-menu-edit-message-menu-btn').should('be.enabled').click();

		// Should open the commit rename drawer
		cy.getByTestId('edit-commit-message-box').should('be.visible');

		const newCommitMessageTitle = 'New Commit Message Title';

		// Should have the original commit message, and be focused
		cy.getByTestId('commit-drawer-title-input')
			.should('be.visible')
			.should('be.enabled')
			.should('be.focused')
			.clear()
			.type(newCommitMessageTitle); // Type the new commit message title

		// Click on the save button
		cy.getByTestId('commit-drawer-action-button')
			.should('be.visible')
			.should('be.enabled')
			.should('contain', 'Save')
			.click();

		const successDataInProgress: CustomChecksData = {
			...data,
			check_runs: [
				{
					...data.check_runs[0],
					conclusion: null,
					status: 'in_progress',
					completed_at: null,
					started_at: new Date(Date.now()).toISOString()
				},
				{
					...data.check_runs[1],
					started_at: new Date(Date.now()).toISOString(),
					status: 'in_progress',
					conclusion: null,
					completed_at: null
				}
			]
		};

		const successData: CustomChecksData = {
			...data,
			check_runs: [
				{
					...data.check_runs[0],
					started_at: new Date(Date.now()).toISOString(),
					status: 'completed',
					conclusion: 'success',
					completed_at: new Date().toISOString()
				},
				{
					...data.check_runs[1],
					started_at: new Date(Date.now()).toISOString(),
					status: 'completed',
					conclusion: 'success',
					completed_at: new Date().toISOString()
				}
			]
		};

		let pollAfterForcePush = 0;

		cy.intercept(
			{
				method: 'GET',
				url: `https://api.github.com/repos/example/repo/commits/${mockBackend.topBranchName}/check-runs`
			},
			(req) => {
				pollAfterForcePush++;
				if (pollAfterForcePush > 1) {
					req.reply({
						statusCode: 200,
						body: successData
					});
					return;
				}
				req.reply({
					statusCode: 200,
					body: successDataInProgress
				});
			}
		).as('getChecksWithActualChecks');

		cy.getByTestId('stack-push-button').first().should('be.visible').should('be.enabled').click();
		cy.getByTestId('stack-confirm-push-modal-button')
			.should('be.visible')
			.should('be.enabled')
			.click();

		cy.wait(['@getChecksWithActualChecks', '@getChecksWithActualChecks'], { timeout: 11000 })
			.spread((first, second) => {
				expect(first.response.body).to.deep.equal(successDataInProgress);
				expect(second.response.body).to.deep.equal(successData);
			})
			.then(() => {
				cy.getByDataValue('pr-text', 'Passed').should('be.visible');
			});

		cy.wait(10 * 1000).then(() => {
			cy.get('@getChecksWithActualChecks.all').should('have.length', 4);
			cy.get('@getChecksBottom.all').should('have.length', 0);
		}); // Wait for the checks to be fetched
	});
});
