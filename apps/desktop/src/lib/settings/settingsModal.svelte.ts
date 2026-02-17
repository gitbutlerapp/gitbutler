import { UI_STATE } from "$lib/state/uiState.svelte";
import { inject } from "@gitbutler/core/context";
import type { GeneralSettingsPageId } from "$lib/settings/generalSettingsPages";
import type { ProjectSettingsPageId } from "$lib/settings/projectSettingsPages";
import type {
	GeneralSettingsModalState,
	ProjectSettingsModalState,
} from "$lib/state/uiState.svelte";

export function useSettingsModal() {
	const uiState = inject(UI_STATE);

	function openGeneralSettings(selectedId?: GeneralSettingsPageId) {
		const modalState: GeneralSettingsModalState = {
			type: "general-settings",
			selectedId,
		};
		uiState.global.modal.set(modalState);
	}

	function openProjectSettings(projectId: string, selectedId?: ProjectSettingsPageId) {
		const modalState: ProjectSettingsModalState = {
			type: "project-settings",
			projectId,
			selectedId,
		};
		uiState.global.modal.set(modalState);
	}

	function closeSettings() {
		uiState.global.modal.set(undefined);
	}

	return {
		openGeneralSettings,
		openProjectSettings,
		closeSettings,
	};
}
