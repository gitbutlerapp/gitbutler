import { clearCommandMocks, mockCommand } from './support';
import MockBackend from './support/mock/backend';
import { PROJECT_ID } from './support/mock/projects';
import { MOCK_STACK_A_ID } from './support/mock/stacks';

describe('Commit Actions', () => {
	let mockBackend: MockBackend;

	beforeEach(() => {
		mockBackend = new MockBackend();
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('update_commit_message', (params) => mockBackend.updateCommitMessage(params));
		mockCommand('changes_in_worktree', (params) => mockBackend.getWorktreeChanges(params));
		mockCommand('tree_change_diffs', (params) => mockBackend.getDiff(params));
		mockCommand('changes_in_commit', (params) => mockBackend.getCommitChanges(params));
		mockCommand('create_commit_from_worktree_changes', (params) =>
			mockBackend.createCommit(params)
		);

		cy.visit('/');

		cy.url({ timeout: 3000 }).should('include', `/${PROJECT_ID}/workspace/${MOCK_STACK_A_ID}`);
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('Should rename a commit', () => {
		const originalCommitMessage = 'Initial commit';

		const newCommitMessageTitle = 'New commit message title';
		const newCommitMessageBody = 'New commit message body';

		cy.spy(mockBackend, 'updateCommitMessage').as('updateCommitMessageSpy');
		cy.spy(mockBackend, 'getDiff').as('getDiffSpy');

		// Click on the first commit
		cy.getByTestId('commit-row').first().should('contain', originalCommitMessage).click();

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
			stackId: mockBackend.stackId,
			commitOid: mockBackend.commitOid,
			message: `${newCommitMessageTitle}\n\n${newCommitMessageBody}`
		});

		// Should never get the diff information, because there are no partial changes being committed.
		expect(mockBackend.getDiff).to.have.callCount(0);
	});

	it('Should be able to commit', () => {
		const newCommitMessage = 'New commit message';
		const newCommitMessageBody = 'New commit message body';

		// spies
		cy.spy(mockBackend, 'getDiff').as('getDiffSpy');

		// There should be uncommitted changes
		cy.getByTestId('uncommitted-changes-file-list').should('be.visible');

		const fileNames = mockBackend.getWorktreeChangesFileNames();

		expect(fileNames).to.have.length(1);

		const fileName = fileNames[0]!;

		cy.getByTestId('uncommitted-changes-file-list-item')
			.first()
			.should('be.visible')
			.should('contain', fileName);

		// Click on the commit button
		cy.getByTestId('start-commit-button').should('be.visible').should('be.enabled').click();

		// Should open the new commit drawer
		cy.getByTestId('new-commit-drawer').should('be.visible');

		// Should have the "Your commit goes here" text
		cy.getByTestId('your-commit-goes-here').should('be.visible').should('have.class', 'first');

		// Should have selected the file
		cy.getByTestId('uncommitted-changes-file-list-item')
			.first()
			.get('input[type="checkbox"]')
			.should('be.checked');

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

		// Should display the commit rows
		cy.getByTestId('commit-row').should('have.length', 2);

		// Should commit and select the new commit
		cy.getByTestId('commit-drawer-title').should('contain', newCommitMessage);
		cy.getByTestId('commit-drawer-description').should('contain', newCommitMessageBody);

		// Should never get the diff information, because there are no partial changes being committed.
		expect(mockBackend.getDiff).to.have.callCount(0);
	});
});

describe('Commit Actions with no stacks', () => {
	let mockBackend: MockBackend;

	beforeEach(() => {
		mockBackend = new MockBackend({ initalStacks: [] });
		mockCommand('stacks', () => mockBackend.getStacks());
		mockCommand('create_virtual_branch', () => mockBackend.createBranch());
		mockCommand('canned_branch_name', () => mockBackend.getCannedBranchName());
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('update_commit_message', (params) => mockBackend.updateCommitMessage(params));
		mockCommand('changes_in_worktree', (params) => mockBackend.getWorktreeChanges(params));
		mockCommand('tree_change_diffs', (params) => mockBackend.getDiff(params));
		mockCommand('changes_in_commit', (params) => mockBackend.getCommitChanges(params));
		mockCommand('create_commit_from_worktree_changes', (params) =>
			mockBackend.createCommit(params)
		);

		cy.visit('/');

		cy.url({ timeout: 3000 }).should('include', `/${PROJECT_ID}/workspace`);
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('Should be able to commit even without a stack present', () => {
		const newCommitMessage = 'New commit message';
		const newCommitMessageBody = 'New commit message body';

		// spies
		cy.spy(mockBackend, 'getDiff').as('getDiffSpy');
		cy.spy(mockBackend, 'createBranch').as('createBranchSpy');

		// There should be uncommitted changes
		cy.getByTestId('uncommitted-changes-file-list').should('be.visible');

		const fileNames = mockBackend.getWorktreeChangesFileNames();

		expect(fileNames).to.have.length(1);

		const fileName = fileNames[0]!;

		cy.getByTestId('uncommitted-changes-file-list-item')
			.first()
			.should('be.visible')
			.should('contain', fileName);

		// Click on the commit button
		cy.getByTestId('start-commit-button').should('be.visible').should('be.enabled').click();

		// Should open the new commit drawer
		cy.getByTestId('new-commit-drawer').should('be.visible');

		// Should display the draft stack
		cy.getByTestId('stack-draft').should('be.visible');
		cy.getByTestId('stack-draft').should('contain', mockBackend.cannedBranchName);

		// Should have the "Your commit goes here" text
		cy.getByTestId('your-commit-goes-here').should('be.visible').should('have.class', 'draft');

		// Should have selected the file
		cy.getByTestId('uncommitted-changes-file-list-item')
			.first()
			.get('input[type="checkbox"]')
			.should('be.checked');

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

		// Should display the commit rows
		cy.getByTestId('commit-row').should('have.length', 1);

		// Should commit and select the new commit
		cy.getByTestId('commit-drawer-title').should('contain', newCommitMessage);
		cy.getByTestId('commit-drawer-description').should('contain', newCommitMessageBody);

		// Should never get the diff information, because there are no partial changes being committed.
		expect(mockBackend.getDiff).to.have.callCount(0);
	});
});
