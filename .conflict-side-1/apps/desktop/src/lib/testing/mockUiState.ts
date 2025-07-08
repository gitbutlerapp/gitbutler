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
	selection: MOCK_UI_SELECTION
};

const MOCK_PROJECT_UI_STATE: ProjectUiState = {
	commitTitle: '',
	commitDescription: '',
	exclusiveAction: undefined,
	branchesSelection: { branchName: 'test' },
	stackId: undefined,
	showActions: false
};

const MOCK_GLOBAL_UI_STATE: GlobalUiState = {
	drawerHeight: 20,
	historySidebarWidth: 30,
	aiSuggestionsOnType: true,
	channel: undefined,
	draftBranchName: undefined,
	useFloatingCommitBox: false,
	useFloatingPrBox: false,
	unassignedSidebaFolded: false,
	floatingCommitPosition: 'bottom-center',
	floatingCommitWidth: 640,
	floatingCommitHeight: 330,
	useRuler: false,
	rulerCountValue: 0,
	wrapTextByRuler: false,
	modal: undefined,
	stackWidth: 22.5,
	detailsWidth: 25,
	previewWidth: 30,
	branchesViewSidebarWidth: 40
};

export function getUiStateMock() {
	const UiStateMock = vi.fn();

	UiStateMock.prototype.global = {
		drawerHeight: {
			get() {
				return MOCK_GLOBAL_UI_STATE.drawerHeight;
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
