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
	upstream: false,
	previewOpen: false
};

const MOCK_STACK_UI_STATE: StackState = {
	selection: MOCK_UI_SELECTION,
	newCommitMessage: { title: '', description: '' },
	prompt: '',
	permissionMode: 'default',
	disabledMcpServers: [],
	addedDirs: []
};

const MOCK_PROJECT_UI_STATE: ProjectUiState = {
	exclusiveAction: undefined,
	branchesToPoll: [],
	selectedClaudeSession: undefined,
	thinkingLevel: 'normal',
	selectedModel: 'sonnet'
};

const MOCK_GLOBAL_UI_STATE: GlobalUiState = {
	drawerHeight: 20,
	aiSuggestionsOnType: true,
	channel: undefined,
	draftBranchName: undefined,
	useFloatingBox: false,
	unassignedSidebarFolded: false,
	floatingBoxSize: { width: 640, height: 330 },
	floatingBoxPosition: 'bottom-center',
	useRuler: true,
	rulerCountValue: 0,
	modal: undefined,
	stackWidth: 22.5,
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
			newCommitMessage: {
				get() {
					return MOCK_STACK_UI_STATE.newCommitMessage;
				}
			}
		};
	});

	return UiStateMock;
}
