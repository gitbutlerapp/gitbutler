import { vi } from 'vitest';
import type {
	StackUiState,
	ProjectUiState,
	GlobalUiState,
	StackUiSelection
} from '$lib/state/uiState.svelte';

const MOCK_UI_SELECTION: StackUiSelection = {
	branchName: 'branch-a',
	commitId: 'commit-a-id',
	upstream: false
};

const MOCK_STACK_UI_STATE: StackUiState = {
	selection: MOCK_UI_SELECTION,
	activeSelectionId: { type: 'worktree' }
};

const MOCK_PROJECT_UI_STATE: ProjectUiState = {
	drawerPage: 'branch',
	drawerFullScreen: false,
	commitTitle: '',
	commitMessage: ''
};

const MOCK_GLOBAL_UI_STATE: GlobalUiState = {
	drawerHeight: 20,
	leftWidth: 17.5,
	stacksViewWidth: 21.25,
	drawerSplitViewWidth: 20,
	useRichText: true,
	aiSuggestionsOnType: true
};

export function getUiStateMock() {
	const UiStateMock = vi.fn();

	UiStateMock.prototype.global = {
		drawerHeight: {
			get() {
				return MOCK_GLOBAL_UI_STATE.drawerHeight;
			}
		},
		leftWidth: {
			get() {
				return MOCK_GLOBAL_UI_STATE.leftWidth;
			}
		},
		stacksViewWidth: {
			get() {
				return MOCK_GLOBAL_UI_STATE.stacksViewWidth;
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
					return MOCK_PROJECT_UI_STATE.drawerPage;
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
					return MOCK_PROJECT_UI_STATE.commitMessage;
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
			},
			activeSelectionId: {
				get() {
					return MOCK_STACK_UI_STATE.activeSelectionId;
				}
			}
		};
	});

	return UiStateMock;
}
