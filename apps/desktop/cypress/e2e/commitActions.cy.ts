import { clearCommandMocks, mockCommand } from './support';
import MockBackend from './support/mock/backend';
import { PROJECT_ID } from './support/mock/projects';
import BranchesWithChanges from './support/scenarios/branchesWithChanges';
import LotsOfFileChanges from './support/scenarios/lotsOfFileChanges';
import StackWithTwoEmptyBranches from './support/scenarios/stackWithTwoEmptyBranches';

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
		cy.getByTestId('commit-drawer-title').first().should('contain', originalCommitMessage);

		// Click on the kebab menu to access edit message
		cy.getByTestId('commit-drawer').within(() => {
			cy.getByTestId('kebab-menu-btn').click();
		});

		// Click on the edit message button in the context menu
		cy.getByTestId('commit-row-context-menu-edit-message-menu-btn').should('be.enabled').click();

		// Should open the commit rename drawer
		cy.getByTestId('edit-commit-message-box').should('be.visible');

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
			.clear()
			.type(newCommitMessageBody); // Type the new commit message body

		// Click on the save button
		cy.getByTestId('commit-drawer-action-button')
			.should('be.visible')
			.should('be.enabled')
			.should('contain', 'Save')
			.click();

		cy.getByTestId('edit-commit-message-box').should('not.exist');

		cy.getByTestId('commit-drawer-title').should('contain', newCommitMessageTitle);
		cy.getByTestId('commit-drawer-description').should('contain', newCommitMessageBody);

		// Should call the update commit message function
		cy.get('@updateCommitMessageSpy').should('be.calledWith', {
			projectId: PROJECT_ID,
			stackId: mockBackend.stackId,
			commitId: mockBackend.commitId,
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
		cy.getByTestId('commit-drawer-title').first().should('contain', originalCommitMessage);

		// Click on the kebab menu to access edit message
		cy.getByTestId('commit-drawer').within(() => {
			cy.getByTestId('kebab-menu-btn').click();
		});

		// Click on the edit message button in the context menu
		cy.getByTestId('commit-row-context-menu-edit-message-menu-btn').should('be.enabled').click();

		// Should open the commit rename drawer
		cy.getByTestId('edit-commit-message-box').should('be.visible');

		// Should have the original commit message, and be focused
		cy.getByTestId('commit-drawer-title-input')
			.should('have.value', originalCommitMessage)
			.should('be.visible')
			.should('be.focused')
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

		cy.getByTestId('edit-commit-message-box').should('not.exist');

		cy.getByTestId('commit-drawer-title').should('contain', originalCommitMessage);
		cy.getByTestId('commit-drawer-description').should('contain', newCommitDescription);

		// Should call the update commit message function
		cy.get('@updateCommitMessageSpy').should('be.calledWith', {
			projectId: PROJECT_ID,
			stackId: mockBackend.stackId,
			commitId: mockBackend.commitId,
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
		cy.getByTestId('commit-drawer-title').first().should('contain', originalCommitMessage);

		// Click on the kebab menu to access edit message
		cy.getByTestId('commit-drawer').within(() => {
			cy.getByTestId('kebab-menu-btn').click();
		});

		// Click on the edit message button in the context menu
		cy.getByTestId('commit-row-context-menu-edit-message-menu-btn').should('be.enabled').click();

		// Should open the commit rename drawer
		cy.getByTestId('edit-commit-message-box').should('be.visible');

		// Should have the original commit message, and be focused
		cy.getByTestId('commit-drawer-title-input')
			.should('have.value', originalCommitMessage)
			.should('be.visible')
			.should('be.enabled')
			.should('be.focused')
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

		cy.getByTestId('edit-commit-message-box').should('not.exist');

		cy.getByTestId('commit-drawer-title').should('contain', newCommitTitle);
		cy.getByTestId('commit-drawer-description').should('not.exist');

		// Should call the update commit message function
		cy.get('@updateCommitMessageSpy').should('be.calledWith', {
			projectId: PROJECT_ID,
			stackId: mockBackend.stackId,
			commitId: mockBackend.commitId,
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
		cy.getByTestId('commit-drawer-title').first().should('contain', originalCommitMessage);

		// Click on the kebab menu to access edit message
		cy.getByTestId('commit-drawer').within(() => {
			cy.getByTestId('kebab-menu-btn').click();
		});

		// Click on the edit message button in the context menu
		cy.getByTestId('commit-row-context-menu-edit-message-menu-btn').should('be.enabled').click();

		// Should open the commit rename drawer
		cy.getByTestId('edit-commit-message-box').should('be.visible');

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

		cy.getByTestId('edit-commit-message-box').should('not.exist');

		cy.getByTestId('commit-drawer-title').should('contain', originalCommitMessage);
		cy.getByTestId('commit-drawer-description').should('not.exist');

		// Should call the update commit message function
		cy.get('@updateCommitMessageSpy').should('be.calledWith', {
			projectId: PROJECT_ID,
			stackId: mockBackend.stackId,
			commitId: mockBackend.commitId,
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
		cy.getByTestId('edit-commit-message-box').should('be.visible');

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
			.clear()
			.type(newCommitMessageBody); // Type the new commit message body

		// Click on the save button
		cy.getByTestId('commit-drawer-action-button')
			.should('be.visible')
			.should('be.enabled')
			.should('contain', 'Save')
			.click();

		cy.getByTestId('edit-commit-message-box').should('not.exist');

		cy.getByTestId('commit-drawer-title').should('contain', newCommitMessageTitle);
		cy.getByTestId('commit-drawer-description').should('contain', newCommitMessageBody);

		// Should call the update commit message function
		cy.get('@updateCommitMessageSpy').should('be.calledWith', {
			projectId: PROJECT_ID,
			stackId: mockBackend.stackId,
			commitId: mockBackend.commitId,
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
		cy.getByTestId('commit-drawer-title').first().should('contain', originalCommitMessage);

		// Click on the kebab menu to access edit message
		cy.getByTestId('commit-drawer').within(() => {
			cy.getByTestId('kebab-menu-btn').click();
		});

		// Click on the edit message button in the context menu
		cy.getByTestId('commit-row-context-menu-edit-message-menu-btn').should('be.enabled').click();

		// Should open the commit rename drawer
		cy.getByTestId('edit-commit-message-box').should('be.visible');

		// Should have the original commit message, and be focused
		cy.getByTestId('commit-drawer-title-input')
			.should('have.value', originalCommitMessage)
			.should('be.visible')
			.should('be.enabled')
			.clear()
			.type(newCommitMessageTitle); // Type the new commit message title

		// Click on the cancel button
		cy.getByTestId('commit-drawer-cancel-button').should('be.visible').should('be.enabled').click();

		cy.getByTestId('edit-commit-message-box').should('not.exist');

		cy.getByTestId('commit-drawer-title').should('contain', originalCommitMessage);
		cy.getByTestId('commit-drawer-description').should('not.exist');

		// Start the message edit again.
		// The commit drawer should be open still.
		cy.getByTestId('commit-drawer-title').first().should('contain', originalCommitMessage);

		// Click on the kebab menu to access edit message again
		cy.getByTestId('commit-drawer').within(() => {
			cy.getByTestId('kebab-menu-btn').click();
		});

		// Click on the edit message button in the context menu
		cy.getByTestId('commit-row-context-menu-edit-message-menu-btn').should('be.enabled').click();

		// Should open the commit rename drawer
		cy.getByTestId('edit-commit-message-box').should('be.visible');

		// Should have the original commit message, and be focused
		cy.getByTestId('commit-drawer-title-input')
			.should('have.value', originalCommitMessage)
			.should('be.visible')
			.should('be.enabled')
			.should('be.focused')
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
			.should('be.focused')
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
		cy.getByTestId('commit-row', newCommitMessage).should('be.visible').click();

		// Should be able to see the commit drawer
		cy.getByTestId('commit-drawer-title').should('contain', newCommitMessage);
		cy.getByTestId('commit-drawer-description').should('contain', newCommitMessageBody);

		// Should never get the diff information, because there are no partial changes being committed.
		expect(mockBackend.getDiff).to.have.callCount(0);
	});

	it('Should be able to commit - even if it takes too long and runs hooks', () => {
		// enable the commit hooks
		cy.window().then((win) => {
			win.localStorage.setItem('projectRunCommitHooks_' + PROJECT_ID, 'true');
		});

		mockCommand('pre_commit_hook_diffspecs', async () => {
			return await mockBackend.precommitHookDiffspecs(2000);
		});

		mockCommand('post_commit_hook', async () => {
			return await mockBackend.postcommitHook(2000);
		});

		mockCommand('create_commit_from_worktree_changes', async (params) => {
			return await new Promise((resolve) => {
				setTimeout(() => {
					resolve(mockBackend.createCommit(params));
				}, 2000); // Simulate a delay of 2 seconds
			});
		});

		const newCommitMessage = 'New commit message';
		const newCommitMessageBody = 'New commit message body';

		// spies
		cy.spy(mockBackend, 'getDiff').as('getDiffSpy');
		cy.spy(mockBackend, 'precommitHookDiffspecs').as('precommitHookDiffspecsSpy');
		cy.spy(mockBackend, 'postcommitHook').as('postcommitHookSpy');

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
			.should('be.focused')
			.type(newCommitMessage); // Type the new commit message

		// Type in a description
		cy.getByTestId('commit-drawer-description-input')
			.should('be.visible')
			// .click()
			.type(newCommitMessageBody); // Type the new commit message body

		// Click on the commit button
		cy.getByTestId('commit-drawer-action-button').should('be.visible').should('be.enabled').click();

		cy.get('@precommitHookDiffspecsSpy').should('have.callCount', 1);

		// Try to click the commit button again
		cy.getByTestId('commit-drawer-action-button').click({ force: true });

		cy.get('@precommitHookDiffspecsSpy').should('have.callCount', 1);

		// Commit button should enter the loading state and be unclickable
		cy.getByTestId('commit-drawer-action-button').should('be.visible').should('be.disabled');

		// Should display the commit rows
		cy.getByTestId('commit-row').should('have.length', 2);
		cy.getByTestId('commit-row', newCommitMessage).should('be.visible').click();

		// Should be able to see the commit drawer
		cy.getByTestId('commit-drawer-title').should('contain', newCommitMessage);
		cy.getByTestId('commit-drawer-description').should('contain', newCommitMessageBody);

		// Should never get the diff information, because there are no partial changes being committed.
		expect(mockBackend.getDiff).to.have.callCount(0);
	});

	it('Should hide the drawer on uncommit from context menu', () => {
		// Click on the first commit and open the commit menu
		cy.getByTestId('commit-row')
			.click()
			.within(() => {
				cy.getByTestId('kebab-menu-btn').click();
			});

		// Click on the uncommit option
		cy.getByTestId('commit-row-context-menu-uncommit-menu-btn').click();

		// The drawer should be closed
		cy.getByTestId('commit-drawer').should('not.exist');

		// The commit should be removed from the list
		cy.getByTestId('commit-row').should('have.length', 0);
	});

	it('Should hide the drawer on uncommit from the commit drawer', () => {
		// Click on the first commit
		cy.getByTestId('commit-row').first().click();

		// Should open the commit drawer
		cy.getByTestId('commit-drawer').first().should('be.visible');

		// Click on the kebab menu to access uncommit
		cy.getByTestId('commit-drawer').within(() => {
			cy.getByTestId('kebab-menu-btn').click();
		});

		// Click on the uncommit button
		cy.getByTestId('commit-row-context-menu-uncommit-menu-btn').click();

		// The drawer should be closed
		cy.getByTestId('commit-drawer').should('not.exist');

		// The commit should be removed from the list
		cy.getByTestId('commit-row').should('have.length', 0);
	});
});

describe('Commit Actions with branches containing changes', () => {
	let mockBackend: BranchesWithChanges;
	beforeEach(() => {
		mockBackend = new BranchesWithChanges();
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
		mockCommand('changes_in_branch', (params) => mockBackend.getBranchChanges(params));

		cy.visit('/');

		cy.urlMatches(`/${PROJECT_ID}/workspace`);
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('should be able to commit in the middle', () => {
		const firsCommitId = mockBackend.firstCommitInSecondStack.id;

		cy.spy(mockBackend, 'createCommit').as('createCommitSpy');

		cy.get(`[data-testid-stackid="${mockBackend.stackWithTwoCommits}"]`)
			.should('be.visible')
			.within(() => {
				// Start committing
				cy.getByTestId('start-commit-button').should('be.visible').should('be.enabled').click();

				// Click where we want to commit
				cy.getByTestId('commit-here-button')
					.should('have.attr', 'data-testid-commit-id', firsCommitId)
					.first()
					.trigger('mouseenter')
					.click();

				// 'Your commit goes here' should be visible at the right position
				cy.getByTestId('your-commit-goes-here')
					.should('be.visible')
					.should('have.attr', 'data-testid-commit-id', firsCommitId);
			});

		const commitTitle = 'New commit title';
		const commitDescription = 'New commit description';

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

		cy.getByTestId('commit-drawer-action-button').should('be.visible').should('be.enabled').click();

		cy.get('@createCommitSpy').should('be.calledWith', {
			projectId: PROJECT_ID,
			parentId: firsCommitId,
			stackId: mockBackend.stackWithTwoCommits,
			message: `${commitTitle}\n\n${commitDescription}`,
			stackBranchName: mockBackend.stackWithTwoCommits,
			worktreeChanges: [
				{
					pathBytes: [102, 105, 108, 101, 68, 46, 116, 120, 116],
					previousPathBytes: null,
					hunkHeaders: [
						{
							oldStart: 2,
							oldLines: 8,
							newStart: 2,
							newLines: 7,
							diff: '@@ -2,8 +2,7 @@\n context line 0\n context line 1\n context line 2\n-context line 3\n-old line to be removed\n+new line added\n context line 4\n context line 5\n context line 6'
						},
						{
							oldStart: 10,
							oldLines: 7,
							newStart: 10,
							newLines: 7,
							diff: '@@ -10,7 +10,7 @@\n context before 1\n context before 2\n context before 3\n-old value\n+updated value\n context after 1\n context after 2\n context after 3'
						}
					]
				},
				{
					pathBytes: [102, 105, 108, 101, 74, 46, 116, 120, 116],
					previousPathBytes: null,
					hunkHeaders: []
				}
			]
		});
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
			cy.getByTestId('commit-drawer').within(() => {
				cy.getByTestId('commit-drawer-title').first().should('contain', commitTitle);
			});

			// Click on the kebab menu to access edit message
			cy.getByTestId('commit-drawer').within(() => {
				cy.getByTestId('kebab-menu-btn').click();
			});

			// Click on the edit message button in the context menu
			cy.getByTestId('commit-row-context-menu-edit-message-menu-btn').should('be.enabled').click();

			// Should open the commit rename drawer
			cy.getByTestId('edit-commit-message-box').should('be.visible');

			// Should have the original commit message
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

			cy.getByTestId('edit-commit-message-box').should('not.exist');

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

			cy.getByTestId('commit-row', commitTitle).should('be.visible').click();
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
			cy.getByTestId('commit-drawer').within(() => {
				cy.getByTestId('commit-drawer-title').first().should('contain', commitTitle);
			});

			// Click on the kebab menu to access edit message
			cy.getByTestId('commit-drawer').within(() => {
				cy.getByTestId('kebab-menu-btn').click();
			});

			// Click on the edit message button in the context menu
			cy.getByTestId('commit-row-context-menu-edit-message-menu-btn').should('be.enabled').click();

			// Should open the commit rename drawer
			cy.getByTestId('edit-commit-message-box').should('be.visible');

			// Should have the original commit message, and be focused
			cy.getByTestId('commit-drawer-title-input')
				.should('have.value', commitTitle)
				.should('be.visible')
				.should('be.enabled')
				.should('be.focused')
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

			cy.getByTestId('edit-commit-message-box').should('not.exist');

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
			cy.getByTestId('commit-drawer').within(() => {
				// Should have the commit title
				cy.getByTestId('commit-drawer-title').first().should('contain', commitTitle);
			});

			// Click on the kebab menu to access edit message
			cy.getByTestId('commit-drawer').within(() => {
				cy.getByTestId('kebab-menu-btn').click();
			});

			// Click on the edit message button in the context menu
			cy.getByTestId('commit-row-context-menu-edit-message-menu-btn').should('be.enabled').click();

			// Should open the commit rename drawer
			cy.getByTestId('edit-commit-message-box').should('be.visible');

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

			cy.getByTestId('edit-commit-message-box').should('not.exist');

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
		mockCommand('create_virtual_branch', (params) => mockBackend.createBranch(params));
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
		cy.spy(mockBackend, 'createCommit').as('createCommitSpy');

		// There should be uncommitted changes
		cy.getByTestId('uncommitted-changes-file-list').should('be.visible');

		const fileNames = mockBackend.getWorktreeChangesFileNames();

		expect(fileNames).to.have.length(1);

		const fileName = fileNames[0]!;

		cy.getByTestId('file-list-item').first().should('be.visible').should('contain', fileName);

		// Click on the commit button
		cy.getByTestId('commit-to-new-branch-button').should('be.visible').should('be.enabled').click();

		// Should open the new commit drawer
		cy.getByTestId('commit-drawer-title-input').should('be.visible');

		// Should display the draft stack
		cy.getByTestId('draft-stack').should('be.visible');
		cy.getByTestId('draft-stack').should('contain', mockBackend.cannedBranchName);

		// Update the stack name
		cy.getByTestId('branch-card').within(() => {
			cy.get('input[type="text"]')
				.should('be.visible')
				.should('be.enabled')
				.click()
				.should('have.value', mockBackend.cannedBranchName)
				.clear()
				.type(newBranchName);
		});

		// Should have the "Your commit goes here" text
		cy.getByTestId('your-commit-goes-here').should('be.visible');

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

		cy.get('@createBranchSpy').should('be.calledWith', {
			projectId: PROJECT_ID,
			branch: {
				name: 'my-cool-branch',
				order: 0
			}
		});

		cy.get('@createCommitSpy').should('be.calledWith', {
			projectId: '1',
			parentId: undefined,
			stackId: 'my-cool-branch',
			message: 'New commit message\n\nNew commit message body',
			stackBranchName: 'my-cool-branch',
			worktreeChanges: [
				{
					pathBytes: [
						47, 112, 97, 116, 104, 47, 116, 111, 47, 112, 114, 111, 106, 101, 99, 116, 65, 47, 102,
						105, 108, 101, 65, 46, 116, 120, 116
					],
					previousPathBytes: null,
					hunkHeaders: []
				}
			]
		});

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

describe('Commit Actions with a stack of two empty branches', () => {
	let mockBackend: StackWithTwoEmptyBranches;

	beforeEach(() => {
		mockBackend = new StackWithTwoEmptyBranches();
		mockCommand('stacks', () => mockBackend.getStacks());
		mockCommand('create_virtual_branch', (params) => mockBackend.createBranch(params));
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

	it('Should be able to commit to the top-most branch', () => {
		// There should be two branches in the stack
		cy.getByTestId('branch-card').should('have.length', 2);
		cy.get(`[data-series-name="${mockBackend.firstBranchName}"]`).should('be.visible');
		cy.get(`[data-series-name="${mockBackend.secondBranchName}"]`).should('be.visible');

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

		// Should have the "Your commit goes here" text only once, in the top-most branch
		cy.get(`[data-series-name="${mockBackend.firstBranchName}"]`).within(() => {
			cy.getByTestId('your-commit-goes-here').should('be.visible').should('have.length', 1);
		});

		// The second branch should not have the "Your commit goes here" text
		cy.get(`[data-series-name="${mockBackend.secondBranchName}"]`).within(() => {
			cy.getByTestId('your-commit-goes-here').should('not.exist');
		});

		// Should commit to the top-most branch
		const commitTitle = `This is a great commit title`;
		const commitDescription = `And look at this amazing commit description\nOh wow, so good!`;

		// Type in a commit message
		cy.getByTestId('commit-drawer-title-input')
			.should('be.visible')
			.should('be.enabled')
			.should('be.focused')
			.should('have.value', '')
			.type(commitTitle); // Type the new commit message

		// Type in a description
		cy.getByTestId('commit-drawer-description-input')
			.should('be.visible')
			.should('contain', '')
			.click()
			.type(commitDescription); // Type the new commit message body

		// Click on the commit button
		cy.getByTestId('commit-drawer-action-button').should('be.visible').should('be.enabled').click();

		// Should display the commit rows in th e top-most branch
		cy.get(`[data-series-name="${mockBackend.firstBranchName}"]`).within(() => {
			cy.getByTestId('commit-row').should('have.length', 1);
			cy.getByTestId('commit-row', commitTitle).should('be.visible');
		});

		// There should be no commits in the second branch
		cy.get(`[data-series-name="${mockBackend.secondBranchName}"]`).within(() => {
			cy.getByTestId('commit-row').should('have.length', 0);
		});
	});
});
