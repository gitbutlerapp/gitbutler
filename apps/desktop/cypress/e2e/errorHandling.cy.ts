import { clearCommandMocks, mockCommand } from './support';
import { PROJECT_ID } from './support/mock/projects';
import BranchesWithChanges from './support/scenarios/branchesWithChanges';

describe('Error handling - commit actions', () => {
	let mockBackend: BranchesWithChanges;

	const UPDATE_COMMIT_ERROR_MESSAGE = 'Error updating commit message';
	const COMMIT_ERROR_MESSAGE = 'Error creating commit';
	const COMMIT_UNDO_ERROR_MESSAGE = 'Error undoing commit';

	beforeEach(() => {
		mockBackend = new BranchesWithChanges();
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('update_commit_message', () => {
			throw new Error(UPDATE_COMMIT_ERROR_MESSAGE);
		});
		mockCommand('changes_in_worktree', (params) => mockBackend.getWorktreeChanges(params));
		mockCommand('tree_change_diffs', (params) => mockBackend.getDiff(params));
		mockCommand('commit_details', (params) => mockBackend.getCommitChanges(params));
		mockCommand('create_commit_from_worktree_changes', () => {
			throw new Error(COMMIT_ERROR_MESSAGE);
		});
		mockCommand('undo_commit', () => {
			throw new Error(COMMIT_UNDO_ERROR_MESSAGE);
		});
		mockCommand('hunk_dependencies_for_workspace_changes', (params) =>
			mockBackend.getHunkDependencies(params)
		);
		mockCommand('hunk_assignments', (params) => mockBackend.getHunkAssignments(params));

		cy.visit('/');

		cy.urlMatches(`/${PROJECT_ID}/workspace`);
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('Failing to rename a commit should fail gracefully', () => {
		const originalCommitMessage = 'Initial commit';

		const newCommitMessageTitle = 'New commit message title';
		const newCommitMessageBody = 'New commit message body';

		cy.spy(mockBackend, 'getDiff').as('getDiffSpy');

		// Click on the first commit
		cy.getByTestId('commit-row').first().should('contain', originalCommitMessage).click();

		// Should open the commit drawer
		cy.get('.commit-view').first().should('contain', originalCommitMessage);

		// Click on the edit message button
		cy.getByTestId('commit-drawer-action-edit-message').should('contain', 'Edit message').click();

		// Should open the commit rename drawer
		cy.getByTestId('edit-commit-message-box').should('be.visible');

		// Should have the original commit message, and be focused
		cy.getByTestId('commit-drawer-title-input')
			.should('have.value', originalCommitMessage)
			.should('be.visible')
			.should('be.enabled')
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

		// Should show the error message
		cy.getByTestId('toast-info-message')
			.should('be.visible')
			.should('contain', UPDATE_COMMIT_ERROR_MESSAGE);

		// Should never get the diff information, because there are no partial changes being committed.
		expect(mockBackend.getDiff).to.have.callCount(0);
	});

	it('Failing to commit should fail gracefully', () => {
		const newCommitMessage = 'New commit message';
		const newCommitMessageBody = 'New commit message body';

		// spies
		cy.spy(mockBackend, 'getDiff').as('getDiffSpy');

		// There should be uncommitted changes
		cy.getByTestId('uncommitted-changes-file-list').should('be.visible');

		const fileNames = mockBackend.getWorktreeChangesFileNames();

		expect(fileNames).to.have.length(2);

		const fileName = fileNames[0]!;

		cy.getByTestId('file-list-item').first().should('be.visible').should('contain', fileName);

		// Click on the commit button
		cy.getByTestId('start-commit-button').should('be.visible').should('be.enabled').click();

		// Should open the new commit drawer
		cy.getByTestId('new-commit-view').should('be.visible');

		// Should have the "Your commit goes here" text
		cy.getByTestId('your-commit-goes-here').should('be.visible').should('have.class', 'first');

		// Should have selected the file
		cy.getByTestId('file-list-item').first().get('input[type="checkbox"]').should('be.checked');

		// Type in a commit message
		cy.getByTestId('commit-drawer-title-input')
			.should('be.visible')
			.should('be.enabled')
			.type(newCommitMessage); // Type the new commit message

		// Type in a description
		cy.getByTestId('commit-drawer-description-input')
			.should('be.visible')
			.click()
			.type(newCommitMessageBody); // Type the new commit message body

		// Click on the commit button
		cy.getByTestId('commit-drawer-action-button').should('be.visible').should('be.enabled').click();

		// Should show the error message
		cy.getByTestId('toast-info-message')
			.should('be.visible')
			.should('contain', COMMIT_ERROR_MESSAGE);

		// Should never get the diff information, because there are no partial changes being committed.
		expect(mockBackend.getDiff).to.have.callCount(0);
	});

	it('Fully failing to commit with rejection reasons shuold be handled graceful', () => {
		mockCommand('create_commit_from_worktree_changes', () =>
			mockBackend.commitFailureWithReasons(null)
		);

		const newCommitMessage = 'New commit message';
		const newCommitMessageBody = 'New commit message body';

		// spies
		cy.spy(mockBackend, 'getDiff').as('getDiffSpy');

		// There should be uncommitted changes
		cy.getByTestId('uncommitted-changes-file-list').should('be.visible');

		const fileNames = mockBackend.getWorktreeChangesFileNames();

		expect(fileNames).to.have.length(2);

		const fileName = fileNames[0]!;

		cy.getByTestId('file-list-item').first().should('be.visible').should('contain', fileName);

		// Click on the commit button
		cy.getByTestId('start-commit-button').should('be.visible').should('be.enabled').click();

		// Should open the new commit drawer
		cy.getByTestId('new-commit-view').should('be.visible');

		// Should have the "Your commit goes here" text
		cy.getByTestId('your-commit-goes-here').should('be.visible').should('have.class', 'first');

		// Should have selected the file
		cy.getByTestId('file-list-item').first().get('input[type="checkbox"]').should('be.checked');

		// Type in a commit message
		cy.getByTestId('commit-drawer-title-input')
			.should('be.visible')
			.should('be.enabled')
			.type(newCommitMessage); // Type the new commit message

		// Type in a description
		cy.getByTestId('commit-drawer-description-input')
			.should('be.visible')
			.click()
			.type(newCommitMessageBody); // Type the new commit message body

		// Click on the commit button
		cy.getByTestId('commit-drawer-action-button').should('be.visible').should('be.enabled').click();

		// Should display the modal with the message
		cy.getByTestId('global-modal-commit-failed').should('be.visible');

		// Should be able to dismiss the modal
		cy.getByTestId('global-modal-action-button').should('be.visible').click();
		cy.getByTestId('global-modal-commit-failed').should('not.exist');

		// The commit drawer should be open still
		cy.getByTestId('new-commit-view').should('be.visible');
		cy.getByTestId('commit-drawer-title-input').should('have.value', newCommitMessage);
		cy.getByTestId('commit-drawer-description-input').should('contain', newCommitMessageBody);
	});

	// PAVELITO: If you want to see the commit failed modal, change the `it(` to `it.only`, that will make it so that
	// this test is the only one run.
	// When you're done with your changes, please change the `it.only` to `it` and remove the comment.
	// I also commented out some lines at the bottom of this test, please uncomment them when you're done.
	it('Partially failing to commit with rejection reasons shuold be handled graceful', () => {
		const newCommitId = '29384726398746289374';
		mockCommand('create_commit_from_worktree_changes', () =>
			mockBackend.commitFailureWithReasons(newCommitId)
		);

		const newCommitMessage = 'New commit message';
		const newCommitMessageBody = 'New commit message body';

		// spies
		cy.spy(mockBackend, 'getDiff').as('getDiffSpy');

		// There should be uncommitted changes
		cy.getByTestId('uncommitted-changes-file-list').should('be.visible');

		const fileNames = mockBackend.getWorktreeChangesFileNames();

		expect(fileNames).to.have.length(2);

		const fileName = fileNames[0]!;

		cy.getByTestId('file-list-item').first().should('be.visible').should('contain', fileName);

		// Click on the commit button
		cy.getByTestId('start-commit-button').should('be.visible').should('be.enabled').click();

		// Should open the new commit drawer
		cy.getByTestId('new-commit-view').should('be.visible');

		// Should have the "Your commit goes here" text
		cy.getByTestId('your-commit-goes-here').should('be.visible').should('have.class', 'first');

		// Should have selected the file
		cy.getByTestId('file-list-item').first().get('input[type="checkbox"]').should('be.checked');

		// Type in a commit message
		cy.getByTestId('commit-drawer-title-input')
			.should('be.visible')
			.should('be.enabled')
			.type(newCommitMessage); // Type the new commit message

		// Type in a description
		cy.getByTestId('commit-drawer-description-input')
			.should('be.visible')
			.click()
			.type(newCommitMessageBody); // Type the new commit message body

		// Click on the commit button
		cy.getByTestId('commit-drawer-action-button').should('be.visible').should('be.enabled').click();

		// Should display the modal with the message
		cy.getByTestId('global-modal-commit-failed').should('be.visible');

		// The commit id should be displayed
		cy.getByTestId('global-modal-commit-failed').should('contain', newCommitId.substring(0, 7));

		// PAVELITO: The following 5 lines should be uncommented when the modal redesign is done, please.

		// // Should be able to dismiss the modal
		// cy.getByTestId('global-modal-action-button').should('be.visible').click();
		// cy.getByTestId('global-modal-commit-failed').should('not.exist');
		// // The commit drawer should be closed
		// cy.getByTestId('new-commit-view').should('not.exist');
	});

	it('Failing to uncommit should fail graceful', () => {
		// Click on the first commit
		cy.getByTestId('commit-row').first().click();

		// Should open the commit drawer
		cy.getByTestId('commit-drawer').first().should('be.visible');

		// Click on the uncommit button
		cy.getByTestId('commit-drawer-action-uncommit').click();

		// Should show the error message
		cy.getByTestId('toast-info-message')
			.should('be.visible')
			.should('contain', COMMIT_UNDO_ERROR_MESSAGE);
	});
});
