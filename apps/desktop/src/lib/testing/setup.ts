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
import { vi } from 'vitest';

export type TestSetup = {
	cleanup: () => void;
	context: Map<any, any>;
	tauri: Tauri;
};

function createContext() {
	const MockStackService = getStackServiceMock();
	const UiStateMock = getUiStateMock();
	const BaseBranchMock = getMockBaseBranch();
	const AIServiceMock = getAIServiceMock();

	const stackService = new MockStackService();
	const baseBranchService = vi.fn();
	const uiState = new UiStateMock();
	const forge = vi.fn();
	const aiService = new AIServiceMock();
	const baseBranch = new BaseBranchMock();

	return new Map<any, any>([
		[StackService, stackService],
		[BaseBranchService, baseBranchService],
		[UiState, uiState],
		[DefaultForgeFactory, forge],
		[AIService, aiService],
		[BaseBranch, baseBranch]
	]);
}
/**
 * Setup the testing environment for component tests.
 */
export function setup(): TestSetup {
	const previousResizeObserver = global.ResizeObserver;
	global.ResizeObserver = class ResizeObserver {
		observe() {
			// do nothing
		}

		unobserve() {
			// do nothing
		}

		disconnect() {
			// do nothing
		}
	};

	const previousIntersectionObserver = global.IntersectionObserver;
	global.IntersectionObserver = class IntersectionObserver {
		observe() {
			// do nothing
		}

		unobserve() {
			// do nothing
		}

		disconnect() {
			// do nothing
		}

		takeRecords() {
			return [];
		}

		root = null;
		rootMargin = '';
		thresholds = [];
	};

	const tauri = new Tauri();

	let context = createContext();

	vi.spyOn(tauri, 'listen').mockReturnValue(async () => {});

	return {
		cleanup: () => {
			global.ResizeObserver = previousResizeObserver;
			global.IntersectionObserver = previousIntersectionObserver;
			context = createContext();
		},
		context,
		tauri
	};
}
