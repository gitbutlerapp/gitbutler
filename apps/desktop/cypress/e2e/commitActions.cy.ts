import { clearCommandMocks, mockCommand } from './support';
import MockBackend from './support/mock/backend';
import { PROJECT_ID } from './support/mock/projects';
import LotsOfFileChanges from './support/scenarios/lotsOfFileChanges';

describe('Commit Actions', () => {
	let mockBackend: MockBackend;

	beforeEach(() => {
		mockBackend = new MockBackend();
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('update_commit_message', (params) => mockBackend.updateCommitMessage(params));
		mockCommand('changes_in_worktree', (params) => mockBackend.getWorktreeChanges(params));
		mockCommand('tree_change_diffs', (params) => mockBackend.getDiff(params));
		mockCommand('hunk_assignments', (params) => mockBackend.getHunkAssignments(params));
		mockCommand('commit_details', (params) => mockBackend.getCommitChanges(params));
		mockCommand('create_commit_from_worktree_changes', (params) =>
			mockBackend.createCommit(params)
		);
		mockCommand('undo_commit', (params) => mockBackend.undoCommit(params));

		cy.visit('/');

		cy.urlMatches(`/${PROJECT_ID}/workspace`);
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
			.clear()
			.type(newCommitMessageTitle); // Type the new commit message title

		// Type in a description
		cy.getByTestId('commit-drawer-description-input')
			.should('be.visible')
			.click()
			.clear()
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

	it('Should be able to edit only the description', () => {
		const originalCommitMessage = 'Initial commit';
		const newCommitDescription = 'New commit message body';

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
			.should('be.enabled');

		// Type in a description
		cy.getByTestId('commit-drawer-description-input')
			.should('be.visible')
			.click()
			.clear()
			.type(newCommitDescription); // Type the new commit message body

		// Click on the save button
		cy.getByTestId('commit-drawer-action-button')
			.should('be.visible')
			.should('be.enabled')
			.should('contain', 'Save')
			.click();

		cy.getByTestId('edit-commit-message-drawer').should('not.exist');

		cy.getByTestId('commit-drawer-title').should('contain', originalCommitMessage);
		cy.getByTestId('commit-drawer-description').should('contain', newCommitDescription);

		// Should call the update commit message function
		cy.get('@updateCommitMessageSpy').should('be.calledWith', {
			projectId: PROJECT_ID,
			stackId: mockBackend.stackId,
			commitOid: mockBackend.commitOid,
			message: `${originalCommitMessage}\n\n${newCommitDescription}`
		});

		// Should never get the diff information, because there are no partial changes being committed.
		expect(mockBackend.getDiff).to.have.callCount(0);
	});

	it('Should be able to edit only the title', () => {
		const originalCommitMessage = 'Initial commit';
		const newCommitTitle = 'New commit message title';

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
			.clear()
			.type(newCommitTitle); // Type the new commit message title

		// Type in a description
		cy.getByTestId('commit-drawer-description-input').should('be.visible').click().clear(); // Clear the description

		// Click on the save button
		cy.getByTestId('commit-drawer-action-button')
			.should('be.visible')
			.should('be.enabled')
			.should('contain', 'Save')
			.click();

		cy.getByTestId('edit-commit-message-drawer').should('not.exist');

		cy.getByTestId('commit-drawer-title').should('contain', newCommitTitle);
		cy.getByTestId('commit-drawer-description').should('not.exist');

		// Should call the update commit message function
		cy.get('@updateCommitMessageSpy').should('be.calledWith', {
			projectId: PROJECT_ID,
			stackId: mockBackend.stackId,
			commitOid: mockBackend.commitOid,
			message: newCommitTitle
		});

		expect(mockBackend.getDiff).to.have.callCount(0);
	});

	it('If nothing is changed, it should call the edit commit message function with the same message', () => {
		const originalCommitMessage = 'Initial commit';

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
			.should('be.enabled');

		// Click on the save button
		cy.getByTestId('commit-drawer-action-button')
			.should('be.visible')
			.should('be.enabled')
			.should('contain', 'Save')
			.click();

		cy.getByTestId('edit-commit-message-drawer').should('not.exist');

		cy.getByTestId('commit-drawer-title').should('contain', originalCommitMessage);
		cy.getByTestId('commit-drawer-description').should('not.exist');

		// Should call the update commit message function
		cy.get('@updateCommitMessageSpy').should('be.calledWith', {
			projectId: PROJECT_ID,
			stackId: mockBackend.stackId,
			commitOid: mockBackend.commitOid,
			message: originalCommitMessage
		});

		expect(mockBackend.getDiff).to.have.callCount(0);
	});

	it('Should be able to rename a commit from the context menu', () => {
		const originalCommitMessage = 'Initial commit';

		const newCommitMessageTitle = 'New commit message title';
		const newCommitMessageBody = 'New commit message body';

		cy.spy(mockBackend, 'updateCommitMessage').as('updateCommitMessageSpy');
		cy.spy(mockBackend, 'getDiff').as('getDiffSpy');

		// Click on the first commit
		cy.getByTestId('commit-row').first().should('contain', originalCommitMessage).rightclick();

		// Should open the context menu
		cy.getByTestId('commit-row-context-menu')
			.should('be.visible')
			.within(() => {
				// Click on the edit message button
				cy.getByTestId('commit-row-context-menu-edit-message-menu-btn')
					.should('be.enabled')
					.click();
			});

		// Should open the commit rename drawer
		cy.getByTestId('edit-commit-message-drawer').should('be.visible');

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
			.clear()
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

	it('Should be able to cancel the commit message edit', () => {
		const originalCommitMessage = 'Initial commit';
		const newCommitMessageTitle = 'New commit message title';

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
			.clear()
			.type(newCommitMessageTitle); // Type the new commit message title

		// Click on the cancel button
		cy.getByTestId('commit-drawer-cancel-button').should('be.visible').should('be.enabled').click();

		cy.getByTestId('edit-commit-message-drawer').should('not.exist');

		cy.getByTestId('commit-drawer-title').should('contain', originalCommitMessage);
		cy.getByTestId('commit-drawer-description').should('not.exist');

		// Start the message edit again.
		// The commit drawer should be open still.
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
			.clear()
			.type(newCommitMessageTitle); // Type the new commit message title
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

		cy.getByTestId('file-list-item').first().should('be.visible').should('contain', fileName);

		// Click on the commit button
		cy.getByTestId('start-commit-button').should('be.visible').should('be.enabled').click();

		// Should open the new commit drawer
		cy.getByTestId('new-commit-view').should('be.visible');

		// Should have the "Your commit goes here" text
		cy.getByTestId('your-commit-goes-here').should('be.visible').should('have.class', 'first');

		// Select the file
		cy.getByTestId('file-list-item').first().get('input[type="checkbox"]').check();

		// Type in a commit message
		cy.getByTestId('commit-drawer-title-input')
			.should('be.visible')
			.should('be.enabled')
			.type(newCommitMessage); // Type the new commit message

		// Type in a description
		cy.getByTestId('commit-drawer-description-input')
			.should('be.visible')
			// .click()
			.type(newCommitMessageBody); // Type the new commit message body

		// Click on the commit button
		cy.getByTestId('commit-drawer-action-button').should('be.visible').should('be.enabled').click();

		// Should display the commit rows
		cy.getByTestId('commit-row').should('have.length', 2);

		cy.getByTestId('commit-row').should('have.length', 2);
		cy.getByTestId('commit-row', newCommitMessage).click();

		// Should commit and select the new commit
		cy.getByTestId('commit-drawer-title').should('contain', newCommitMessage);
		cy.getByTestId('commit-drawer-description').should('contain', newCommitMessageBody);

		// Should never get the diff information, because there are no partial changes being committed.
		expect(mockBackend.getDiff).to.have.callCount(0);
	});

	// it('Should hide the drawer on uncommit from context menu', () => {
	// 	// Click on the first commit and open the commit menu
	// 	cy.getByTestId('commit-row')
	// 		.click()
	// 		.within(() => {
	// 			cy.getByTestId('kebab-menu-btn').click();
	// 		});

	// 	// Click on the uncommit option
	// 	cy.getByTestId('commit-row-context-menu-uncommit-menu-btn').click();

	// 	// The drawer should be closed
	// 	cy.getByTestId('commit-drawer').should('not.exist');

	// 	// The commit should be removed from the list
	// 	cy.getByTestId('commit-row').should('have.length', 0);
	// });

	it('Should hide the drawer on uncommit from the commit drawer', () => {
		// Click on the first commit
		cy.getByTestId('commit-row').first().click();

		// Should open the commit drawer
		cy.getByTestId('commit-drawer').first().should('be.visible');

		// Click on the uncommit button
		cy.getByTestId('commit-drawer-action-uncommit').click();

		// The drawer should be closed
		cy.getByTestId('commit-drawer').should('not.exist');

		// The commit should be removed from the list
		cy.getByTestId('commit-row').should('have.length', 0);
	});
});

describe('Commit Actions with lots of uncommitted changes', () => {
	let mockBackend: LotsOfFileChanges;
	beforeEach(() => {
		mockBackend = new LotsOfFileChanges();
		mockCommand('stacks', () => mockBackend.getStacks());
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('changes_in_worktree', (params) => mockBackend.getWorktreeChanges(params));
		mockCommand('update_commit_message', (params) => mockBackend.updateCommitMessage(params));
		mockCommand('tree_change_diffs', (params) => mockBackend.getDiff(params));
		mockCommand('commit_details', (params) => mockBackend.getCommitChanges(params));
		mockCommand('create_commit_from_worktree_changes', (params) =>
			mockBackend.createCommit(params)
		);
		mockCommand('undo_commit', (params) => mockBackend.undoCommit(params));
		mockCommand('hunk_assignments', (params) => mockBackend.getHunkAssignments(params));

		cy.visit('/');

		cy.urlMatches(`/${PROJECT_ID}/workspace`);
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('should be able to commit a bunch of times in a row and edit their message', () => {
		const TIMES = 3;
		for (let i = 0; i < TIMES; i++) {
			// Click commit button
			cy.getByTestId('start-commit-button').should('be.visible').should('be.enabled').click();

			// There should only be one 'Your commit goes here' text
			cy.getByTestId('your-commit-goes-here')
				.should('have.length', 1)
				.should('be.visible')
				.should('have.class', 'first');

			// Unstage all files
			cy.getByTestId('uncommitted-changes-header')
				.should('be.visible')
				.within(() => {
					cy.get('input[type="checkbox"]').should('be.visible').click();
				});

			// Stage the file
			cy.getByTestId('uncommitted-changes-file-list')
				.should('be.visible')
				.within(() => {
					cy.getByTestId('file-list-item')
						.first()
						.scrollIntoView()
						.should('be.visible')
						.within(() => {
							cy.get('input[type="checkbox"]').should('be.visible').click();
						});
				});

			const commitTitle = `Commit title ${i + 1}`;
			const commitDescription = `Commit description ${i + 1}`;

			// Type in a commit message
			cy.getByTestId('commit-drawer-title-input')
				.should('be.visible')
				.should('be.enabled')
				.should('have.value', '')
				.type(commitTitle); // Type the new commit message

			// Type in a description
			cy.getByTestId('commit-drawer-description-input')
				.should('be.visible')
				.should('contain', '')
				.click()
				.type(commitDescription); // Type the new commit message body

			// Click on the commit button
			cy.getByTestId('commit-drawer-action-button')
				.should('be.visible')
				.should('be.enabled')
				.click();

			cy.getByTestId('commit-row', commitTitle).should('be.visible');
		}

		for (let i = 0; i < TIMES; i++) {
			const commitTitle = `Commit title ${i + 1}`;
			const commitDescription = `Commit description ${i + 1}`;

			const newCommitTitle = `New commit title ${i + 1}`;
			const newCommitDescription = `New commit description ${i + 1}`;

			// Click on the first commit
			cy.getByTestId('commit-row', commitTitle).should('contain', commitTitle).click();

			// Should open the commit drawer
			cy.get('.commit-view').first().should('contain', commitTitle);

			// Click on the edit message button
			cy.getByTestId('commit-drawer-action-edit-message').should('contain', 'Edit message').click();

			// Should open the commit rename drawer
			cy.getByTestId('edit-commit-message-drawer').should('be.visible');

			// Should have the original commit message, and be focused
			cy.getByTestId('commit-drawer-title-input')
				.should('have.value', commitTitle)
				.should('be.visible')
				.should('be.enabled')
				.clear()
				.type(newCommitTitle); // Type the new commit message title

			// Type in a description
			cy.getByTestId('commit-drawer-description-input')
				.should('be.visible')
				.should('contain', commitDescription)
				.click()
				.clear()
				.type(newCommitDescription); // Type the new commit message body

			// Click on the save button
			cy.getByTestId('commit-drawer-action-button')
				.should('be.visible')
				.should('be.enabled')
				.should('contain', 'Save')
				.click();

			cy.getByTestId('edit-commit-message-drawer').should('not.exist');

			cy.getByTestId('commit-drawer-title').should('contain', newCommitTitle);
			cy.getByTestId('commit-drawer-description').should('contain', newCommitDescription);
		}

		for (let i = TIMES; i < TIMES * 2; i++) {
			// Click commit button
			cy.getByTestId('start-commit-button').should('be.visible').should('be.enabled').click();

			// There should only be one 'Your commit goes here' text
			cy.getByTestId('your-commit-goes-here')
				.should('have.length', 1)
				.should('be.visible')
				.should('have.class', 'first');

			// Unstage all files
			cy.getByTestId('uncommitted-changes-header')
				.should('be.visible')
				.within(() => {
					cy.get('input[type="checkbox"]').should('be.visible').click();
				});

			// Stage the file
			cy.getByTestId('uncommitted-changes-file-list')
				.should('be.visible')
				.within(() => {
					cy.getByTestId('file-list-item')
						.first()
						.scrollIntoView()
						.should('be.visible')
						.within(() => {
							cy.get('input[type="checkbox"]').should('be.visible').click();
						});
				});

			const commitTitle = `Commit title ${i + 1}`;
			const commitDescription = `Commit description ${i + 1}`;

			// Type in a commit message
			cy.getByTestId('commit-drawer-title-input')
				.should('be.visible')
				.should('be.enabled')
				.should('have.value', '')
				.type(commitTitle); // Type the new commit message

			// Type in a description
			cy.getByTestId('commit-drawer-description-input')
				.should('be.visible')
				.should('contain', '')
				.click()
				.type(commitDescription); // Type the new commit message body

			// Click on the commit button
			cy.getByTestId('commit-drawer-action-button')
				.should('be.visible')
				.should('be.enabled')
				.click();

			cy.getByTestId('commit-row', commitTitle).should('be.visible');
		}

		// Start editing the commits and cancel
		for (let i = TIMES; i < TIMES * 2; i++) {
			const commitTitle = `Commit title ${i + 1}`;
			const commitDescription = `Commit description ${i + 1}`;

			const newCommitTitle = `New commit title ${i + 1}`;
			const newCommitDescription = `New commit description ${i + 1}`;

			// Click on the first commit
			cy.getByTestId('commit-row', commitTitle).should('contain', commitTitle).click();

			// Should open the commit drawer
			cy.get('.commit-view').first().should('contain', commitTitle);

			// Click on the edit message button
			cy.getByTestId('commit-drawer-action-edit-message').should('contain', 'Edit message').click();

			// Should open the commit rename drawer
			cy.getByTestId('edit-commit-message-drawer').should('be.visible');

			// Should have the original commit message, and be focused
			cy.getByTestId('commit-drawer-title-input')
				.should('have.value', commitTitle)
				.should('be.visible')
				.should('be.enabled')
				.clear()
				.type(newCommitTitle); // Type the new commit message title

			// Type in a description
			cy.getByTestId('commit-drawer-description-input')
				.should('be.visible')
				.should('contain', commitDescription)
				.click()
				.clear()
				.type(newCommitDescription); // Type the new commit message body

			// Click on the cancel button
			cy.getByTestId('commit-drawer-cancel-button')
				.should('be.visible')
				.should('be.enabled')
				.click();

			cy.getByTestId('edit-commit-message-drawer').should('not.exist');

			cy.getByTestId('commit-drawer-title').should('contain', commitTitle);
			cy.getByTestId('commit-drawer-description').should('contain', commitDescription);
		}

		let lastInputTitle: string | undefined = undefined;
		let lastInputDescription: string | undefined = undefined;

		// Start creating a commit and cancel
		for (let i = TIMES * 2; i < TIMES * 3; i++) {
			// Click commit button
			cy.getByTestId('start-commit-button').should('be.visible').should('be.enabled').click();

			// There should only be one 'Your commit goes here' text
			cy.getByTestId('your-commit-goes-here')
				.should('have.length', 1)
				.should('be.visible')
				.should('have.class', 'first');

			// Unstage all files
			cy.getByTestId('uncommitted-changes-header')
				.should('be.visible')
				.within(() => {
					cy.get('input[type="checkbox"]').should('be.visible').click();
				});

			// Stage the file
			cy.getByTestId('uncommitted-changes-file-list')
				.should('be.visible')
				.within(() => {
					cy.getByTestId('file-list-item')
						.first()
						.scrollIntoView()
						.should('be.visible')
						.within(() => {
							cy.get('input[type="checkbox"]').should('be.visible').click();
						});
				});

			const commitTitle = `Commit title ${i + 1}`;
			const commitDescription = `Commit description ${i + 1}`;

			// Type in a commit message
			cy.getByTestId('commit-drawer-title-input')
				.should('be.visible')
				.should('be.enabled')
				.should('have.value', lastInputTitle ?? '')
				.clear()
				.type(commitTitle); // Type the new commit message

			lastInputTitle = commitTitle;

			// Type in a description
			cy.getByTestId('commit-drawer-description-input')
				.should('be.visible')
				.should('contain', lastInputDescription ?? '')
				.click()
				.clear()
				.type(commitDescription); // Type the new commit message body

			lastInputDescription = commitDescription;

			// Click on the cancel button
			cy.getByTestId('commit-drawer-cancel-button')
				.should('be.visible')
				.should('be.enabled')
				.click();

			cy.getByTestId('commit-row', commitTitle).should('not.exist');
		}

		// Edit the commit messages
		for (let i = TIMES; i < TIMES * 2; i++) {
			const commitTitle = `Commit title ${i + 1}`;
			const commitDescription = `Commit description ${i + 1}`;

			const newCommitTitle = `New commit title ${i + 1}`;
			const newCommitDescription = `New commit description ${i + 1}`;

			// Click on the first commit
			cy.getByTestId('commit-row', commitTitle).should('contain', commitTitle).click();

			// Should open the commit drawer
			cy.get('.commit-view').first().should('contain', commitTitle);

			// Click on the edit message button
			cy.getByTestId('commit-drawer-action-edit-message').should('contain', 'Edit message').click();

			// Should open the commit rename drawer
			cy.getByTestId('edit-commit-message-drawer').should('be.visible');

			// Should have the original commit message, and be focused
			cy.getByTestId('commit-drawer-title-input')
				.should('have.value', commitTitle)
				.should('be.visible')
				.should('be.enabled')
				.clear()
				.type(newCommitTitle); // Type the new commit message title

			// Type in a description
			cy.getByTestId('commit-drawer-description-input')
				.should('be.visible')
				.should('contain', commitDescription)
				.click()
				.clear()
				.type(newCommitDescription); // Type the new commit message body

			// Edit the commit message
			cy.getByTestId('commit-drawer-action-button')
				.should('be.visible')
				.should('be.enabled')
				.click();

			cy.getByTestId('edit-commit-message-drawer').should('not.exist');

			cy.getByTestId('commit-drawer-title').should('contain', newCommitTitle);
			cy.getByTestId('commit-drawer-description').should('contain', newCommitDescription);
		}
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
		mockCommand('commit_details', (params) => mockBackend.getCommitChanges(params));
		mockCommand('create_commit_from_worktree_changes', (params) =>
			mockBackend.createCommit(params)
		);
		mockCommand('normalize_branch_name', (params) => {
			if (!params) return '';
			if ('name' in params && typeof params.name === 'string') {
				return params.name;
			}
		});
		mockCommand('hunk_assignments', (params) => mockBackend.getHunkAssignments(params));

		cy.visit('/');

		cy.urlMatches(`/${PROJECT_ID}/workspace`);
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('Should be able to commit even without a stack present', () => {
		const newBranchName = 'my-cool-branch';
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

		cy.getByTestId('file-list-item').first().should('be.visible').should('contain', fileName);

		// Click on the commit button
		cy.getByTestId('commit-to-new-branch-button').should('be.visible').should('be.enabled').click();

		// Should open the new commit drawer
		cy.getByTestId('new-commit-view').should('be.visible');

		// Should display the draft stack
		cy.getByTestId('draft-stack').should('be.visible');
		cy.getByTestId('draft-stack').should('contain', mockBackend.cannedBranchName);

		// Update the stack name
		cy.getByTestId('branch-card').within(() => {
			cy.get('input[type="text"]')
				.should('be.visible')
				.should('be.enabled')
				.clear()
				.type(newBranchName);
		});

		// Should have the "Your commit goes here" text
		cy.getByTestId('your-commit-goes-here').should('be.visible').should('have.class', 'draft');

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

		// Should display the commit rows
		cy.getByTestId('commit-row').should('have.length', 1);

		// Select new commit and validate message.
		cy.getByTestId('commit-row', newCommitMessage).click();
		cy.getByTestId('commit-drawer-title').should('contain', newCommitMessage);
		cy.getByTestId('commit-drawer-description').should('contain', newCommitMessageBody);

		// Should never get the diff information, because there are no partial changes being committed.
		expect(mockBackend.getDiff).to.have.callCount(0);
	});
});
