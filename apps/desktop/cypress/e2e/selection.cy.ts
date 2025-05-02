import { clearCommandMocks, mockCommand } from './support';
import BranchesWithChanges from './support/scenarios/branchesWithChanges';

describe('Selection', () => {
	let mockBackend: BranchesWithChanges;

	beforeEach(() => {
		mockBackend = new BranchesWithChanges();

		mockCommand('stacks', () => mockBackend.getStacks());
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('changes_in_branch', (args) => mockBackend.getBranchChanges(args));

		cy.visit('/');

		cy.url({ timeout: 3000 }).should('include', `/workspace/${mockBackend.stackId}`);
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('should update the drawer when changing stack selection', () => {
		// Stack with branch should  be opened by default
		cy.getByTestId('branch-header').should('contain', mockBackend.stackId);

		const stacks = mockBackend.getStacks();
		// There shuold be three stacks
		cy.getByTestId('stack-tab').should('have.length', stacks.length);

		// Select the second stack

		for (const stack of stacks) {
			const stackName = stack.heads[0]?.name;
			if (!stackName) continue;

			cy.getByTestId('stack-tab', stackName)
				.click()
				.then(() => {
					// Shouls have the stack url
					cy.url().should('include', `/workspace/${stack.id}`);

					// Check if the stack name is displayed in the header
					cy.getByTestId('branch-header').should('contain', stackName).click();

					// Check if the file list is updated
					cy.getByTestId('branch-changed-file-list')
						.should('be.visible')
						.within(() => {
							const changedFileNames = mockBackend.getBranchChangesFileNames(stack.id, stackName);
							for (const fileName of changedFileNames) {
								cy.getByTestId('file-list-item', fileName).should('be.visible');
							}
						});
				});
		}
	});
});
