import { InjectionToken } from "@gitbutler/core/context";
import type { IBackend } from "$lib/backend";
import type { TerminalSettings } from "$lib/state/uiState.svelte";

export const TERMINAL_SERVICE = new InjectionToken<TerminalService>("TerminalService");

export class TerminalService {
	constructor(private backend: IBackend) {}

	async getTerminalOptionsForPlatform(platform: string): Promise<TerminalSettings[]> {
		return await this.backend.invoke("get_terminal_options_for_platform", {
			platform,
		});
	}

	async getRecommendedTerminalForPlatform(platform: string): Promise<TerminalSettings | null> {
		return await this.backend.invoke("get_recommended_terminal_for_platform", {
			platform,
		});
	}
}
