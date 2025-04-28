import { clearCommandMocks, mockCommand } from './support';
import { PROJECT_ID } from './support/mock/projects';
import { MockStackService } from './support/mock/stacks';

describe('Commit Actions', () => {
	let mockStackService: MockStackService;
	beforeEach(() => {
		mockStackService = new MockStackService();
		mockCommand('stack_details', (params) => mockStackService.getStackDetails(params));
		mockCommand('update_commit_message', (params) => mockStackService.updateCommitMessage(params));
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('Should rename a commit', () => {
		const originalCommitMessage = 'Initial commit';

		const newCommitMessageTitle = 'New commit message title';
		const newCommitMessageBody = 'New commit message body';

		cy.spy(mockStackService, 'updateCommitMessage').as('updateCommitMessageSpy');
		cy.visit('/');

		// Click on the first commit
		cy.get('.commit-name').first().should('contain', originalCommitMessage).click();

		// Should open the commit drawer
		cy.get('.commit-view').first().should('contain', originalCommitMessage);

		// Click on the edit message button
		cy.getByTestId('commit-drawer-action-edit-message').should('contain', 'Edit message').click();

		// Should open the commit rename drawer
		cy.getByTestId('edit-commit-message-drawer').should('be.visible');

		// Should have the original commit message, and be focused
		cy.getByTestId('commit-drawer-title-input')
			.should('have.value', originalCommitMessage)
			.should('be.visible')
			.should('be.enabled')
			.should('be.focused')
			.clear()
			.type(newCommitMessageTitle); // Type the new commit message title

		// Type in a description
		cy.getByTestId('commit-drawer-description-input')
			.should('be.visible')
			.click()
			.type(newCommitMessageBody); // Type the new commit message body

		// Click on the save button
		cy.getByTestId('commit-drawer-action-button')
			.should('be.visible')
			.should('be.enabled')
			.should('contain', 'Save')
			.click();

		cy.getByTestId('edit-commit-message-drawer').should('not.exist');

		cy.getByTestId('commit-drawer-title').should('contain', newCommitMessageTitle);
		cy.getByTestId('commit-drawer-description').should('contain', newCommitMessageBody);

		// Should call the update commit message function
		cy.get('@updateCommitMessageSpy').should('be.calledWith', {
			projectId: PROJECT_ID,
			stackId: mockStackService.stackId,
			commitOid: mockStackService.commitOid,
			message: `${newCommitMessageTitle}\n\n${newCommitMessageBody}`
		});
	});
});
