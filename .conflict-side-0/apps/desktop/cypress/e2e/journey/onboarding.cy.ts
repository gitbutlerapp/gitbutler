import { clearCommandMocks, clearMockPlatform, mockCommand, setMockPlatform } from '../support';
import FreshStart from '../support/scenarios/freshStart';

describe('Onboarding Journey', () => {
	let mockBackend: FreshStart;
	beforeEach(() => {
		mockBackend = new FreshStart();

		mockCommand('get_base_branch_data', (args) => mockBackend.getBaseBranchData(args));
		mockCommand('set_base_branch', (args) => mockBackend.setBaseBranch(args));
		mockCommand('list_projects', () => mockBackend.listProjects());
		mockCommand('add_project', (args) => mockBackend.addProject(args));
		mockCommand('get_project', (args) => mockBackend.getProject(args));
		mockCommand('stacks', () => mockBackend.getStacks());
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('changes_in_branch', (args) => mockBackend.getBranchChanges(args));
		mockCommand('integrate_upstream_commits', (args) => mockBackend.integrateUpstreamCommits(args));
		mockCommand('update_branch_name', (params) => mockBackend.renameBranch(params));
		mockCommand('remove_branch', (params) => mockBackend.removeBranch(params));
		mockCommand('create_branch', (params) => mockBackend.addBranch(params));
		mockCommand('hunk_assignments', (params) => mockBackend.getHunkAssignments(params));

		const TEST_PROJECT_PATH = '/Users/butler/Documents/Projects/GitButler/test-project';

		mockCommand('plugin:dialog|open', async () => await Promise.resolve(TEST_PROJECT_PATH));

		cy.visit('/');
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('should not explode - and be able to add a project', () => {
		// The onboarding page should be visible
		cy.getByTestId('onboarding-page').should('be.visible');

		// The analytics settings should be visible
		cy.getByTestId('onboarding-page-analytics-settings').should('be.visible');

		// The error reporting toggle should be visible and checked by default
		cy.getByTestId('onboarding-page-analytics-settings-error-reporting-toggle')
			.should('be.visible')
			.should('be.checked');

		// The telemetry toggle should be visible and checked by default
		cy.getByTestId('onboarding-page-analytics-settings-telemetry-toggle')
			.should('be.visible')
			.should('be.checked');

		// The non-anonymous metrrcs toggle should be visible and unchecked by default
		cy.getByTestId('onboarding-page-analytics-settings-non-anonymous-toggle')
			.should('be.visible')
			.should('not.be.checked');

		// The continue button should be visible
		cy.getByTestId('analytics-continue').should('be.visible').should('be.enabled').click();

		// The welcome page should be visible
		cy.getByTestId('welcome-page').should('be.visible');

		// Click on adding a local project
		cy.getByTestId('add-local-project').should('be.visible').should('be.enabled').click();

		// Should see the set target branch page
		cy.getByTestId('project-setup-page').should('be.visible');
		cy.getByTestId('project-setup-page-target-branch-select').should('be.visible');
		cy.getByTestId('set-base-branch').should('be.visible').click();

		// Should load the project directly after setting base branch
		cy.urlMatches(`/${mockBackend.projectId}/workspace`);
	});
});

describe('Onboarding Journey - Windows', () => {
	let mockBackend: FreshStart;
	beforeEach(() => {
		mockBackend = new FreshStart();

		mockCommand('get_base_branch_data', (args) => mockBackend.getBaseBranchData(args));
		mockCommand('set_base_branch', (args) => mockBackend.setBaseBranch(args));
		mockCommand('list_projects', () => mockBackend.listProjects());
		mockCommand('add_project', (args) => mockBackend.addProject(args));
		mockCommand('get_project', (args) => mockBackend.getProject(args));
		mockCommand('stacks', () => mockBackend.getStacks());
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('changes_in_branch', (args) => mockBackend.getBranchChanges(args));
		mockCommand('integrate_upstream_commits', (args) => mockBackend.integrateUpstreamCommits(args));
		mockCommand('update_branch_name', (params) => mockBackend.renameBranch(params));
		mockCommand('remove_branch', (params) => mockBackend.removeBranch(params));
		mockCommand('create_branch', (params) => mockBackend.addBranch(params));
		mockCommand('hunk_assignments', (params) => mockBackend.getHunkAssignments(params));

		setMockPlatform('windows');

		cy.visit('/');
	});

	afterEach(() => {
		clearCommandMocks();
		clearMockPlatform();
	});

	it.only('should not explode - and be able to add a project', () => {
		const TEST_PROJECT_PATH = '/Users/butler/Documents/Projects/GitButler/test-project';

		mockCommand('plugin:dialog|open', async () => await Promise.resolve(TEST_PROJECT_PATH));

		// The onboarding page should be visible
		cy.getByTestId('onboarding-page').should('be.visible');

		// The analytics settings should be visible
		cy.getByTestId('onboarding-page-analytics-settings').should('be.visible');

		// The error reporting toggle should be visible and checked by default
		cy.getByTestId('onboarding-page-analytics-settings-error-reporting-toggle')
			.should('be.visible')
			.should('be.checked');

		// The telemetry toggle should be visible and checked by default
		cy.getByTestId('onboarding-page-analytics-settings-telemetry-toggle')
			.should('be.visible')
			.should('be.checked');

		// The non-anonymous metrrcs toggle should be visible and unchecked by default
		cy.getByTestId('onboarding-page-analytics-settings-non-anonymous-toggle')
			.should('be.visible')
			.should('not.be.checked');

		// The continue button should be visible
		cy.getByTestId('analytics-continue').should('be.visible').should('be.enabled').click();

		// The welcome page should be visible
		cy.getByTestId('welcome-page').should('be.visible');

		// Click on adding a local project
		cy.getByTestId('add-local-project').should('be.visible').should('be.enabled').click();

		// Should see the set target branch page
		cy.getByTestId('project-setup-page').should('be.visible');
		cy.getByTestId('project-setup-page-target-branch-select').should('be.visible');
		cy.getByTestId('set-base-branch').should('be.visible').click();

		// Should load the project
		cy.urlMatches(`/${mockBackend.projectId}/workspace`);
	});
});
