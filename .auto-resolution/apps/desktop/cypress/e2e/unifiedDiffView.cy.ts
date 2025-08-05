import { clearCommandMocks, mockCommand } from './support';
import BranchesWithChanges from './support/scenarios/branchesWithChanges';
import ComplexHunks from './support/scenarios/complexHunks';

describe('Unified Diff View', () => {
	let mockBackend: BranchesWithChanges;

	beforeEach(() => {
		mockBackend = new BranchesWithChanges();

		mockCommand('stacks', () => mockBackend.getStacks());
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('changes_in_branch', (args) => mockBackend.getBranchChanges(args));
		mockCommand('changes_in_worktree', (params) => mockBackend.getWorktreeChanges(params));
		mockCommand('tree_change_diffs', (params) => mockBackend.getDiff(params));
		mockCommand('hunk_assignments', (params) => mockBackend.getHunkAssignments(params));

		cy.visit('/');
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('should show dependency locks when viewing diffs and hide them when in commit mode', () => {
		// There should be uncommitted changes
		cy.getByTestId('uncommitted-changes-file-list')
			.should('be.visible')
			.within(() => {
				// All files should be visible
				cy.getByTestId('file-list-item').should(
					'have.length',
					mockBackend.getWorktreeChangesFileNames().length
				);
			});

		// Stack with branch should be opened by default
		cy.getByTestId('branch-header').should('contain', mockBackend.stackId);

		const stacks = mockBackend.getStacks();
		// There should be three stacks
		cy.getByTestId('stack').should('have.length', stacks.length);

		// Select the first stack
		expect(stacks.length).to.be.greaterThan(0);
		const stack = stacks[0];
		if (!stack) return;

		expect(stack.heads.length).to.be.greaterThan(0);
		const stackName = stack.heads[0]?.name;
		if (!stackName) return;

		cy.getByTestIdByValue('branch-header', stackName)
			.should('contain', stackName)
			.click()
			.then(() => {
				cy.getByTestIdByValue('stack', stack.id)
					.should('be.visible')
					.within(() => {
						// Check if the file list is updated
						const changedFileNames = mockBackend.getBranchChangesFileNames(stack.id, stackName);
						for (const fileName of changedFileNames) {
							cy.getByTestId('file-list-item', fileName).should('be.visible').click();
						}
					});
			});

		// The unified diff view should be opened when clicking on the uncommitted file
		cy.getByTestId('uncommitted-changes-file-list').within(() => {
			const fileName = mockBackend.getWorktreeChangesFileNames()[0];
			if (!fileName) return;

			cy.getByTestId('file-list-item', fileName).click();
		});

		// The unified diff view should be visible and show locks before entering commit mode
		cy.getByTestId('unified-diff-view')
			.first()
			.should('be.visible')
			.within(() => {
				// The line locks should be visible when not in commit mode
				cy.get('[data-testid="hunk-line-locking-info"]')
					.should('have.length', 5)
					.first()
					.trigger('mouseenter');
			});

		// The tooltip should be visible
		cy.getByTestId('unified-diff-view-lock-warning').should('be.visible');

		// Click on the commit button to enter commit mode
		cy.getByTestId('start-commit-button').first().click();

		// The unified diff view should be visible but locks should be hidden in commit mode
		cy.getByTestId('unified-diff-view')
			.first()
			.should('be.visible')
			.within(() => {
				// The line locks should NOT be visible when in commit mode
				cy.get('[data-testid="hunk-line-locking-info"]').should('not.exist');
			});

		// Cancel the commit.
		cy.getByTestId('commit-drawer-cancel-button').scrollIntoView().should('be.visible').click();

		// Select the stack that the file belongs to
		cy.get(`[data-id="${mockBackend.dependsOnStack}"]`)
			.scrollIntoView()
			.should('be.visible')
			.click();

		// The unified diff view should be opened when clicking on the uncommitted file
		cy.getByTestId('uncommitted-changes-file-list').click();

		// The unified diff view should be visible
		cy.getByTestId('unified-diff-view')
			.first()
			.should('be.visible')
			.within(() => {
				// The line locks should not be visible
				cy.get('[data-testid="hunk-line-locking-info"]').should('not.exist');
			});
	});

	it('should hide big diffs by default', () => {
		// There should be uncommitted changes
		cy.getByTestId('uncommitted-changes-file-list').should('be.visible');

		// All files should be visible
		cy.getByTestId('file-list-item').should(
			'have.length',
			mockBackend.getWorktreeChangesFileNames().length
		);

		// Open bif file diff
		cy.getByTestId('uncommitted-changes-file-list').within(() => {
			const fileName = mockBackend.bigFileName;
			cy.getByTestId('file-list-item', fileName).click();
		});

		cy.getByTestId('unified-diff-view').within(() => {
			// The diff should not be visible
			cy.get('table').should('not.exist');
		});

		// The large diff message should be visible
		cy.getByTestId('large-diff-message')
			.should('be.visible')
			.within(() => {
				// The large diff message should be visible
				cy.getByTestId('large-diff-message-button').click();
			});

		// The diff should be visible
		cy.getByTestId('unified-diff-view').within(() => {
			// The diff should be visible
			cy.get('table').should('be.visible');
		});
	});

	it('should display the correct option in the hunk context menu for the branch changes', () => {
		// Select a branch
		cy.getByTestId('branch-header', mockBackend.stackId).should('be.visible').click();

		// The branch drawer should be opened.
		// Open a file from the list changes.
		cy.getByTestId('stack')
			.first()
			.should('be.visible')
			.within(() => {
				cy.getByTestId('file-list-item').should('have.length', 3).first().click();
			});

		// The unified diff view should be opened.
		cy.getByTestId('unified-diff-view')
			.should('be.visible')
			.within(() => {
				// Right click on the first hunk
				cy.get('[data-testid="hunk-count-column"]').first().rightclick();
			});

		// The hunk context menu should be opened
		cy.getByTestId('hunk-context-menu')
			.should('be.visible')
			.within(() => {
				// The discard change option should not be visible
				cy.getByTestId('hunk-context-menu-discard-change').should('not.exist');
				// The discard lines option should not be visible
				cy.getByTestId('hunk-context-menu-discard-lines').should('not.exist');
				// The open in editor option should be visible
				cy.getByTestId('hunk-context-menu-open-in-editor').should('be.visible');
			});
	});

	it('should display the correct option in the hunk context menu for the uncommitted changes', () => {
		// There should be uncommitted changes
		cy.getByTestId('uncommitted-changes-file-list').should('be.visible');

		// All files should be visible
		cy.getByTestId('file-list-item').should(
			'have.length',
			mockBackend.getWorktreeChangesFileNames().length
		);

		// Open bif file diff
		cy.getByTestId('uncommitted-changes-file-list').within(() => {
			cy.getByTestId('file-list-item').first().click();
		});

		cy.getByTestId('unified-diff-view').within(() => {
			// Right click on the first hunk
			cy.get('[data-testid="hunk-count-column"]').first().rightclick();
		});

		// The hunk context menu should be opened
		cy.getByTestId('hunk-context-menu')
			.should('be.visible')
			.within(() => {
				// The discard change option should be visible
				cy.getByTestId('hunk-context-menu-discard-change').should('be.visible');
				// The open in editor option should be visible
				cy.getByTestId('hunk-context-menu-open-in-editor').should('be.visible');
			});
	});

	it.only('should display the correct option in the hunk context menu for the uncommitted changes with selected lines', () => {
		// There should be uncommitted changes
		cy.getByTestId('uncommitted-changes-file-list').should('be.visible');

		// All files should be visible
		cy.getByTestId('file-list-item').should(
			'have.length',
			mockBackend.getWorktreeChangesFileNames().length
		);

		// Stack B needs to be selected so we can select locked lines.
		cy.getByTestId('branch-header', mockBackend.dependsOnStack).should('be.visible').click();

		// Open bif file diff
		cy.getByTestId('uncommitted-changes-file-list').within(() => {
			cy.getByTestId('file-list-item').first().click();
		});

		cy.getByTestId('unified-diff-view')
			.first()
			.within(() => {
				// Select the first line in the hunk
				// and then right click on it.
				cy.get('[data-is-delta-line=true]').first().click().rightclick();
			});

		// The hunk context menu should be opened
		cy.getByTestId('hunk-context-menu')
			.should('be.visible')
			.within(() => {
				// The discard change option should be visible
				cy.getByTestId('hunk-context-menu-discard-change').should('be.visible');
				// The discard lines option should be visible
				cy.getByTestId('hunk-context-menu-discard-lines').should('be.visible');
				// The open in editor option should be visible
				cy.getByTestId('hunk-context-menu-open-in-editor').should('be.visible');
			});
	});
});

describe('Unified Diff View with complex hunks', () => {
	let mockBackend: ComplexHunks;

	beforeEach(() => {
		mockBackend = new ComplexHunks();

		mockCommand('stacks', () => mockBackend.getStacks());
		mockCommand('stack_details', (params) => mockBackend.getStackDetails(params));
		mockCommand('changes_in_branch', (args) => mockBackend.getBranchChanges(args));
		mockCommand('changes_in_worktree', (params) => mockBackend.getWorktreeChanges(params));
		mockCommand('tree_change_diffs', (params) => mockBackend.getDiff(params));
		mockCommand('create_commit_from_worktree_changes', (params) =>
			mockBackend.createCommit(params)
		);
		mockCommand('hunk_assignments', (params) => mockBackend.getHunkAssignments(params));

		cy.visit('/');
	});

	afterEach(() => {
		clearCommandMocks();
	});

	// TODO(mattias): @estib could you help me fix this? Disabling for now.
	it.skip('should select the hunks correctly in the complex file', () => {
		// spy
		cy.spy(mockBackend, 'createCommit').as('createCommit');

		// There should be uncommitted changes
		cy.getByTestId('uncommitted-changes-file-list').should('be.visible');

		// Click on start a commit
		cy.getByTestId('start-commit-button').first().click();

		// Unstage everything
		cy.getByTestId('uncommitted-changes-header').within(() => {
			cy.get('input[type="checkbox"]').should('be.checked').click();
		});

		// All files should be visible
		cy.getByTestId('uncommitted-changes-file-list').within(() => {
			cy.getByTestId('file-list-item').should(
				'have.length',
				mockBackend.getWorktreeChangesFileNames().length
			);
		});

		// Open big file diff
		cy.getByTestId('uncommitted-changes-file-list').within(() => {
			const fileName = mockBackend.complexHunkFileName;
			cy.getByTestId('file-list-item', fileName).click();
		});

		cy.getByTestId('unified-diff-view').within(() => {
			// The diff should be visible
			cy.get('table').should('be.visible');

			for (const lineSelector of mockBackend.hunkLineSelectorsComplex) {
				cy.get(lineSelector).within(() => {
					cy.get('[data-testid="hunk-count-column"]').first().click({ force: true });
				});
			}
		});

		// Commit the things
		cy.getByTestId('commit-drawer-title-input').should('be.visible').type('Test commit');
		cy.getByTestId('commit-drawer-action-button').should('be.visible').click();

		cy.get('@createCommit').should('be.calledWith', {
			projectId: '1',
			parentId: undefined,
			stackId: 'stack-a-id',
			message: 'Test commit',
			stackBranchName: 'stack-a-id',
			worktreeChanges: mockBackend.expectedWorktreeChangesComplex
		});
	});

	// TODO(mattias): @estib could you help me fix this? Disabling for now.
	it.skip('should select the hunk lines correctly in the long hunk file', () => {
		// spy
		cy.spy(mockBackend, 'createCommit').as('createCommit');

		// There should be uncommitted changes
		cy.getByTestId('uncommitted-changes-file-list').should('be.visible');

		// Click on start a commit
		cy.getByTestId('start-commit-button').first().click();

		// Unstage everything
		cy.getByTestId('uncommitted-changes-header').within(() => {
			cy.get('input[type="checkbox"]').should('be.checked').click();
		});

		// Open the long hunk file
		cy.getByTestId('uncommitted-changes-file-list').within(() => {
			const fileName = mockBackend.longFileName;
			cy.getByTestId('file-list-item', fileName).click();
			// All files should be visible
			cy.getByTestId('file-list-item').should(
				'have.length',
				mockBackend.getWorktreeChangesFileNames().length
			);
		});

		cy.getByTestId('unified-diff-view').within(() => {
			// The diff should be visible
			cy.get('table').should('be.visible');

			for (const lineSelector of mockBackend.hunkLineSelectorsLong) {
				cy.get(lineSelector).within(() => {
					cy.get('[data-testid="hunk-count-column"]').first().click();
				});
			}
		});

		// Commit the things
		cy.getByTestId('commit-drawer-title-input').should('be.visible').type('Test commit');
		cy.getByTestId('commit-drawer-action-button').should('be.visible').click();

		cy.get('@createCommit').should('be.calledWith', {
			projectId: '1',
			parentId: undefined,
			stackId: 'stack-a-id',
			message: 'Test commit',
			stackBranchName: 'stack-a-id',
			worktreeChanges: mockBackend.expectedWorktreeChangesLong
		});
	});

	it('should unselect all when canceling the commit', () => {
		// There should be uncommitted changes
		cy.getByTestId('uncommitted-changes-file-list').should('be.visible');

		// Click on start a commit
		cy.getByTestId('start-commit-button').first().click();

		// Unstage everything
		cy.getByTestId('uncommitted-changes-header')
			.first()
			.within(() => {
				cy.get('input[type="checkbox"]').should('be.checked').click();
			});

		// Open big file diff
		cy.getByTestId('uncommitted-changes-file-list').within(() => {
			const fileName = mockBackend.complexHunkFileName;
			cy.getByTestId('file-list-item', fileName).click();
			// All files should be visible
			cy.getByTestId('file-list-item').should(
				'have.length',
				mockBackend.getWorktreeChangesFileNames().length
			);
		});

		cy.getByTestId('unified-diff-view').within(() => {
			// The diff should be visible
			cy.get('table').should('be.visible');

			for (const lineSelector of mockBackend.hunkLineSelectorsComplex) {
				cy.get(lineSelector).within(() => {
					cy.get('[data-testid="hunk-count-column"]').first().click({ force: true });
				});
			}
		});

		cy.getByTestId('commit-drawer-cancel-button').should('be.visible').click();

		cy.getByTestId('unified-diff-view').within(() => {
			// The diff should be visible
			cy.get('table').should('be.visible');

			for (const lineSelector of mockBackend.hunkLineSelectorsComplex) {
				cy.get(lineSelector).should('be.visible').should('have.attr', 'data-test-staged', 'false');
			}
		});
	});

	it('should deselect only the line that was clicked in the hunk', () => {
		// spy
		cy.spy(mockBackend, 'createCommit').as('createCommit');

		// There should be uncommitted changes
		cy.getByTestId('uncommitted-changes-file-list').should('be.visible');

		// Click on start a commit
		cy.getByTestId('start-commit-button').first().click();

		// Unstage everything expet the long hunk file
		cy.getByTestId('uncommitted-changes-file-list').within(() => {
			cy.getByTestId('file-list-item').each((item) => {
				const fileName = item.text().trim();
				if (fileName !== mockBackend.longFileName) {
					cy.wrap(item).find('input[type="checkbox"]').should('be.checked').click();
				}
			});
		});

		// Click on the long hunk file
		cy.getByTestId('uncommitted-changes-file-list').within(() => {
			const fileName = mockBackend.longFileName;
			cy.getByTestId('file-list-item', fileName).click();
		});

		// Click on the fist staged line in the hunk
		cy.get('[data-test-staged="true"] > [data-is-delta-line="true"]')
			.first()
			.should('be.visible')
			.click();

		// Commit the things
		cy.getByTestId('commit-drawer-title-input').should('be.visible').type('Test commit');
		cy.getByTestId('commit-drawer-action-button').should('be.visible').click();

		cy.get('@createCommit').should('be.calledWith', {
			projectId: '1',
			parentId: undefined,
			stackId: 'stack-a-id',
			message: 'Test commit',
			stackBranchName: 'stack-a-id',
			worktreeChanges: mockBackend.expectedHunkDeselectOneLine
		});
	});
});
