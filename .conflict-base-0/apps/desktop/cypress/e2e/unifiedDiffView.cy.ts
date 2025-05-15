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
		mockCommand('hunk_dependencies_for_workspace_changes', (params) =>
			mockBackend.getHunkDependencies(params)
		);

		cy.visit('/');
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('should open the unified diff view when clicking on a file and show the dependency locks', () => {
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
				cy.getByTestId('branch-changed-file-list')
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

		// Click on the commit button
		cy.getByTestId('start-commit-button').click();

		// The unified diff view should be visible
		cy.getByTestId('unified-diff-view')
			.should('be.visible')
			.within(() => {
				// The line locks shold be visible
				cy.get('[data-testid="hunk-line-locking-info"]')
					.should('have.length', 5)
					.first()
					.trigger('mouseenter');
			});

		// The tooltip should be visible
		cy.getByTestId('unified-diff-view-lock-warning').should('be.visible');

		// Select the stack that the file belongs to
		cy.getByTestId('branch-header', mockBackend.dependsOnStack).should('be.visible').click();

		// The unified diff view should be opened when clicking on the uncommitted file
		cy.getByTestId('uncommitted-changes-file-list').click();

		// The unified diff view should be visible
		cy.getByTestId('unified-diff-view')
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
		mockCommand('hunk_dependencies_for_workspace_changes', (params) =>
			mockBackend.getHunkDependencies(params)
		);
		mockCommand('create_commit_from_worktree_changes', (params) =>
			mockBackend.createCommit(params)
		);

		cy.visit('/');
	});

	afterEach(() => {
		clearCommandMocks();
	});

	it('should select the hunks correctly', () => {
		// spy
		cy.spy(mockBackend, 'createCommit').as('createCommit');

		// There should be uncommitted changes
		cy.getByTestId('uncommitted-changes-file-list').should('be.visible');

		// Click on start a commit
		cy.getByTestId('start-commit-button').click();

		// Unstage everything
		cy.getByTestId('uncommitted-changes-header').within(() => {
			cy.get('input[type="checkbox"]').should('be.checked').click();
		});

		// All files should be visible
		cy.getByTestId('file-list-item').should(
			'have.length',
			mockBackend.getWorktreeChangesFileNames().length
		);

		// Open bif file diff
		cy.getByTestId('uncommitted-changes-file-list').within(() => {
			const fileName = mockBackend.complexHunkFileName;
			cy.getByTestId('file-list-item', fileName).click();
		});

		cy.getByTestId('unified-diff-view').within(() => {
			// The diff should be visible
			cy.get('table').should('be.visible');

			for (const lineSelector of mockBackend.hunkLineSelectors) {
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
			worktreeChanges: mockBackend.expectedWorktreeChanges
		});
	});
});
