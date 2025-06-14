import { vi } from 'vitest';
import type {
	StackState,
	ProjectUiState,
	GlobalUiState,
	StackSelection
} from '$lib/state/uiState.svelte';

const MOCK_UI_SELECTION: StackSelection = {
	branchName: 'branch-a',
	commitId: 'commit-a-id',
	upstream: false
};

const MOCK_STACK_UI_STATE: StackState = {
	selection: MOCK_UI_SELECTION,
	action: undefined
};

const MOCK_PROJECT_UI_STATE: ProjectUiState = {
	drawerFullScreen: false,
	commitTitle: '',
	commitDescription: '',
	exclusiveAction: undefined,
	branchesSelection: { branchName: 'test' },
	stackId: undefined,
	editingCommitMessage: false
};

const MOCK_GLOBAL_UI_STATE: GlobalUiState = {
	drawerHeight: 20,
	drawerSplitViewWidth: 20,
	historySidebarWidth: 30,
	useRichText: true,
	aiSuggestionsOnType: true,
	selectedTip: undefined,
	channel: undefined,
	draftBranchName: undefined,
	useRuler: false,
	rulerCountValue: 0,
	wrapTextByRuler: false,
	modal: undefined,
	stackWidth: 25,
	detailsWidth: 25,
	previewWidth: 30
};

export function getUiStateMock() {
	const UiStateMock = vi.fn();

	UiStateMock.prototype.global = {
		drawerHeight: {
			get() {
				return MOCK_GLOBAL_UI_STATE.drawerHeight;
			}
		},
		drawerSplitViewWidth: {
			get() {
				return MOCK_GLOBAL_UI_STATE.drawerSplitViewWidth;
			}
		},
		useRichText: {
			get() {
				return MOCK_GLOBAL_UI_STATE.useRichText;
			}
		},
		aiSuggestionsOnType: {
			get() {
				return MOCK_GLOBAL_UI_STATE.aiSuggestionsOnType;
			}
		}
	};

	UiStateMock.prototype.project = vi.fn(() => {
		return {
			drawerPage: {
				get() {
					return MOCK_PROJECT_UI_STATE.exclusiveAction;
				}
			},
			drawerFullScreen: {
				get() {
					return MOCK_PROJECT_UI_STATE.drawerFullScreen;
				}
			},
			commitTitle: {
				get() {
					return MOCK_PROJECT_UI_STATE.commitTitle;
				}
			},
			commitMessage: {
				get() {
					return MOCK_PROJECT_UI_STATE.commitDescription;
				}
			}
		};
	});

	UiStateMock.prototype.stack = vi.fn(() => {
		return {
			selection: {
				get() {
					return MOCK_STACK_UI_STATE.selection;
				}
			}
		};
	});

	return UiStateMock;
}
