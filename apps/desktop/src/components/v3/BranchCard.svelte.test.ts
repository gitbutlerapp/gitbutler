import BranchCard from '$components/v3/BranchCard.svelte';
import { AIService } from '$lib/ai/service';
import { Tauri } from '$lib/backend/tauri';
import { BaseBranch } from '$lib/baseBranch/baseBranch';
import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
import { StackService } from '$lib/stacks/stackService.svelte';
import { UiState } from '$lib/state/uiState.svelte';
import { getAIServiceMock } from '$lib/testing/mockAIService';
import { getMockBaseBranch } from '$lib/testing/mockBaseBranch';
import { getStackServiceMock } from '$lib/testing/mockStackService';
import { getUiStateMock } from '$lib/testing/mockUiState';
import { setup } from '$lib/testing/setup';
import { TestId } from '$lib/testing/testIds';
import { clearMocks } from '@tauri-apps/api/mocks';
import { render, waitFor } from '@testing-library/svelte/svelte5';
import userEvent from '@testing-library/user-event';
import { expect, test, describe, beforeEach, afterEach, vi } from 'vitest';

describe('BranchCard Component', () => {
	let cleanup: () => void;

	let tauri: Tauri;
	let stackService: ReturnType<typeof vi.fn>;
	let baseBranchService: ReturnType<typeof vi.fn>;
	let uiState: ReturnType<typeof vi.fn>;
	let forge: ReturnType<typeof vi.fn>;
	let aiService: ReturnType<typeof vi.fn>;
	let baseBranch: ReturnType<typeof vi.fn>;

	let context: Map<any, any>;

	beforeEach(() => {
		cleanup = setup();

		tauri = new Tauri();
		const MockStackService = getStackServiceMock();
		const UiStateMock = getUiStateMock();
		const BaseBranchMock = getMockBaseBranch();
		const AIServiceMock = getAIServiceMock();

		stackService = new MockStackService();
		baseBranchService = vi.fn();
		uiState = new UiStateMock();
		forge = vi.fn();
		aiService = new AIServiceMock();
		baseBranch = new BaseBranchMock();

		context = new Map<any, any>([
			[StackService, stackService],
			[BaseBranchService, baseBranchService],
			[UiState, uiState],
			[DefaultForgeFactory, forge],
			[AIService, aiService],
			[BaseBranch, baseBranch]
		]);
		vi.spyOn(tauri, 'listen').mockReturnValue(async () => {});
	});

	afterEach(() => {
		vi.restoreAllMocks();
		clearMocks();

		cleanup();
	});

	test('should render the BranchCard component and update it correctly', async () => {
		const StackServiceMock = getStackServiceMock();

		// Override the stack service in the context to inject spies 🕵🏻‍♂️
		const branchByNameMock = vi.spyOn(StackServiceMock.prototype, 'branchByName');
		const branchesMock = vi.spyOn(StackServiceMock.prototype, 'branches');
		const branchDetailsMock = vi.spyOn(StackServiceMock.prototype, 'branchDetails');
		const commitAtMock = vi.spyOn(StackServiceMock.prototype, 'commitAt');
		const commitsMock = vi.spyOn(StackServiceMock.prototype, 'commits');
		const upstreamCommitsMock = vi.spyOn(StackServiceMock.prototype, 'upstreamCommits');

		const stackServiceMock = new StackServiceMock();
		context.set(StackService, stackServiceMock);

		const branchName = 'branch-a';
		const { getByTestId, rerender } = render(BranchCard, {
			context,
			props: {
				projectId: 'test-project',
				stackId: 'test-stack',
				branchName,
				first: true,
				last: true
			}
		});

		await waitFor(
			() => {
				const branchLabel = getByTestId(TestId.BranchNameLabel);
				expect(branchLabel).toBeInTheDocument();
				expect(branchLabel).toHaveTextContent(branchName);
			},
			{ timeout: 5000 }
		);

		expect(branchByNameMock).toHaveBeenCalledWith('test-project', 'test-stack', branchName);
		expect(branchByNameMock).toHaveBeenCalledTimes(1);

		expect(branchesMock).toHaveBeenCalledWith('test-project', 'test-stack');
		expect(branchesMock).toHaveBeenCalledTimes(1);

		expect(branchDetailsMock).toHaveBeenCalledWith('test-project', 'test-stack', branchName);
		expect(branchDetailsMock).toHaveBeenCalledTimes(1);

		expect(commitAtMock).toHaveBeenCalledWith('test-project', 'test-stack', branchName, 0);

		expect(commitsMock).toHaveBeenCalledWith('test-project', 'test-stack', branchName);

		expect(upstreamCommitsMock).toHaveBeenCalledWith('test-project', 'test-stack', branchName);

		// Update the branch name props should trigger all stack service calls again

		await rerender({
			projectId: 'test-project',
			stackId: 'test-stack',
			branchName: 'branch-b',
			first: true,
			last: true
		});

		expect(branchByNameMock).toHaveBeenCalledWith('test-project', 'test-stack', 'branch-b');
		expect(branchByNameMock).toHaveBeenCalledTimes(2);

		expect(branchesMock).toHaveBeenCalledWith('test-project', 'test-stack');
		// Since none of the branches parameters changed, the branches call should not be triggered again
		expect(branchesMock).toHaveBeenCalledTimes(1);

		expect(branchDetailsMock).toHaveBeenCalledWith('test-project', 'test-stack', 'branch-b');
		expect(branchDetailsMock).toHaveBeenCalledTimes(2);

		expect(commitAtMock).toHaveBeenCalledWith('test-project', 'test-stack', 'branch-b', 0);
		expect(commitAtMock.mock.calls).toMatchSnapshot();

		expect(commitsMock).toHaveBeenCalledWith('test-project', 'test-stack', 'branch-b');
		expect(commitsMock.mock.calls).toMatchSnapshot();

		expect(upstreamCommitsMock).toHaveBeenCalledWith('test-project', 'test-stack', 'branch-b');
		expect(upstreamCommitsMock.mock.calls).toMatchSnapshot();
	});

	test('should display the context menu on right click', async () => {
		const user = userEvent.setup();

		const { getByTestId, queryByTestId } = render(BranchCard, {
			context,
			props: {
				projectId: 'test-project',
				stackId: 'test-stack',
				branchName: 'branch-a',
				first: true,
				last: true
			}
		});

		const branchHeader = getByTestId(TestId.BranchHeader);
		expect(branchHeader).toBeInTheDocument();

		await user.pointer({ keys: '[MouseRight]', target: branchHeader });

		const contextMenu = getByTestId(TestId.BranchHeaderContextMenu);
		expect(contextMenu).toBeInTheDocument();
		expect(contextMenu).toBeVisible();

		// Verify context menu items
		const addDependentBranchItem = getByTestId(TestId.BranchHeaderContextMenu_AddDependentBranch);
		expect(addDependentBranchItem).toBeInTheDocument();

		// Should not be able to open in browser
		const openInBrowserItem = queryByTestId(TestId.BranchHeaderContextMenu_OpenInBrowser);
		expect(openInBrowserItem).toBeNull();

		const copyBranchNameItem = getByTestId(TestId.BranchHeaderContextMenu_CopyBranchName);
		expect(copyBranchNameItem).toBeInTheDocument();

		// Should not be able to add/remove description
		const addRemoveDescriptionItem = queryByTestId(
			TestId.BranchHeaderContextMenu_AddRemoveDescription
		);
		expect(addRemoveDescriptionItem).toBeNull();

		// Should not be able to generate branch name
		const generateBranchNameItem = queryByTestId(TestId.BranchHeaderContextMenu_GenerateBranchName);
		expect(generateBranchNameItem).toBeNull();

		const renameItem = getByTestId(TestId.BranchHeaderContextMenu_Rename);
		expect(renameItem).toBeInTheDocument();

		// Should not be able to delete, since it's the only branch
		const deleteItem = queryByTestId(TestId.BranchHeaderContextMenu_Delete);
		expect(deleteItem).toBeNull();

		// Should not be able to open PR in browser, sice there's no PR
		const openPRInBrowserItem = queryByTestId(TestId.BranchHeaderContextMenu_OpenPRInBrowser);
		expect(openPRInBrowserItem).toBeNull();

		// Should not be able to copy PR link, since there's no PR
		const copyPRLinkItem = queryByTestId(TestId.BranchHeaderContextMenu_CopyPRLink);
		expect(copyPRLinkItem).toBeNull();
	});
});
