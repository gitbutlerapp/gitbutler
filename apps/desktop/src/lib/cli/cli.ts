// cli.ts, lol

import { InjectionToken } from '@gitbutler/core/context';
import type { BackendApi } from '$lib/state/clientState.svelte';

export const CLI_MANAGER = new InjectionToken<CLIManager>('CLIManager');

export default class CLIManager {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	get install() {
		return this.api.endpoints.installCLI.useMutation();
	}

	path() {
		return this.api.endpoints.cliPath.useQuery();
	}
}

function injectEndpoints(backendApi: BackendApi) {
	return backendApi.injectEndpoints({
		endpoints: (build) => ({
			installCLI: build.mutation<void, void>({
				extraOptions: { command: 'install_cli' },
				query: () => ({})
			}),
			cliPath: build.query<string, void>({
				extraOptions: { command: 'cli_path' },
				query: () => ({})
			})
		})
	});
}
