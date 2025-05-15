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

		cy.visit('/');

		cy.url({ timeout: 3000 }).should('include', `/${PROJECT_ID}/workspace`);
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
});
