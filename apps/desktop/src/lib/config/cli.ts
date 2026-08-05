// cli.ts, lol

import { invalidatesList, providesList, ReduxTag } from "$lib/state/tags";
import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/backendApi";
import type { CliInstallState } from "@gitbutler/but-sdk";

export const CLI_MANAGER = new InjectionToken<CLIManager>("CLIManager");

export default class CLIManager {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	get install() {
		return this.api.endpoints.installCLI.useMutation();
	}

	get uninstall() {
		return this.api.endpoints.uninstallCLI.useMutation();
	}

	path() {
		return this.api.endpoints.cliPath.useQuery();
	}

	state() {
		return this.api.endpoints.cliInstallState.useQuery();
	}

	/** One-shot read for the startup check; see AgentsService.fetchStatus. */
	async fetchState() {
		return await this.api.endpoints.cliInstallState.fetch(undefined, { forceRefetch: true });
	}
}

function injectEndpoints(backendApi: BackendApi) {
	return backendApi.injectEndpoints({
		endpoints: (build) => ({
			installCLI: build.mutation<void, void>({
				extraOptions: { command: "install_cli" },
				query: () => ({}),
				invalidatesTags: () => [invalidatesList(ReduxTag.CliInstallState)],
			}),
			uninstallCLI: build.mutation<CliInstallState, void>({
				extraOptions: { command: "uninstall_cli" },
				query: () => ({}),
				invalidatesTags: () => [invalidatesList(ReduxTag.CliInstallState)],
			}),
			cliInstallState: build.query<CliInstallState, void>({
				extraOptions: { command: "cli_install_state" },
				query: () => ({}),
				providesTags: [providesList(ReduxTag.CliInstallState)],
			}),
			cliPath: build.query<string, void>({
				extraOptions: { command: "cli_path" },
				query: () => ({}),
			}),
		}),
	});
}
