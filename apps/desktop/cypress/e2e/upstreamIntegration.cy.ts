import { clearCommandMocks, mockCommand } from './support';
import { PROJECT_ID } from './support/mock/projects';
import PartiallyIntegratedBranches from './support/scenarios/partialllyIntegratedBranches';

describe('Upstream Integration', () => {
	let mockBackend: PartiallyIntegratedBranches;

	beforeEach(() => {
		mockBackend = new PartiallyIntegratedBranches();
		mockCommand('stacks', () => mockBackend.getStacks());
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('get_base_branch_data', () => mockBackend.getBaseBranchData());
		mockCommand('upstream_integration_statuses', () =>
			mockBackend.getUpstreamIntegrationStatuses()
		);
		mockCommand('integrate_upstream', (params) => mockBackend.integrateUpstream(params));

		cy.visit('/');

		cy.url({ timeout: 3000 }).should('include', `/${PROJECT_ID}/workspace`);
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('Should open the upstream integration modal', () => {
		// The first stack should be shown in the workspace
		cy.getByTestId('branch-header')
			.first()
			.should('be.visible')
			.should('contain', mockBackend.stackId);

		// Click on the "Integrate Upstream Commits" button
		cy.getByTestId('integrate-upstream-commits-button').click();

		// The modal should be visible
		cy.getByTestId('integrate-upstream-commits-modal').should('be.visible');

		// The modal should contain the three stacks
		cy.getByTestId('integrate-upstream-commits-modal')
			.getByTestId('integrate-upstream-series-row')
			.should('have.length', 3);
	});

	it('Should correctly delete the integrated stacks', () => {
		// spy
		cy.spy(mockBackend, 'integrateUpstream').as('integrateUpstream');

		// Click on the "Integrate Upstream Commits" button
		cy.getByTestId('integrate-upstream-commits-button').click();

		// The modal should be visible
		cy.getByTestId('integrate-upstream-commits-modal').should('be.visible');

		// Click on the "Delete" button for the first stack
		cy.getByTestId('integrate-upstream-series-row', 'Delete all local branches')
			.first()
			.find('input[type="checkbox"]')
			.uncheck();

		// Integrate the sh*t out of the upstream
		cy.getByTestId('integrate-upstream-action-button').click();
		cy.getByTestId('integrate-upstream-commits-modal').should('not.exist');

		cy.get('@integrateUpstream').should('have.been.calledWithMatch', {
			projectId: PROJECT_ID,
			resolutions: [
				{
					branchId: 'stack-a-id',
					approach: { type: 'rebase' },
					deleteIntegratedBranches: true
				},
				{
					branchId: 'stack-b-id',
					approach: { type: 'delete' },
					deleteIntegratedBranches: false
				},
				{
					branchId: 'stack-c-id',
					approach: {
						type: 'delete'
					},
					deleteIntegratedBranches: true
				}
			],
			branchResolution: undefined
		});
	});
});
