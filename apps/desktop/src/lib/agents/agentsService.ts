import { invalidatesList, providesList, ReduxTag } from "$lib/state/tags";
import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/backendApi";
import type { AgentsStatus, PolicyOptions, PolicyState, SkillScope } from "@gitbutler/but-sdk";

export const AGENTS_SERVICE = new InjectionToken<AgentsService>("AgentsService");

type ScopedArgs = { scope: SkillScope; projectId?: string };
type FrameworkArgs = ScopedArgs & { frameworkId: string };

/**
 * Agent skill install state, and the workflow policy that gets written into
 * agent instruction files.
 *
 * Every command works without a project: the global settings page manages
 * `$HOME` skills before any repository is open, so `projectId` is optional
 * throughout and simply narrows results to a repository when given.
 */
export class AgentsService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	status(args: { projectId?: string }) {
		return this.api.endpoints.agentsStatus.useQuery(args);
	}

	/**
	 * One-shot read for the startup check.
	 *
	 * Deliberately not `useQuery`: a component that renders nothing would hold
	 * a live subscription forever, and an effect reading it would fire once
	 * while still loading and again with data.
	 */
	async fetchStatus(args: { projectId?: string }) {
		return await this.api.endpoints.agentsStatus.fetch(args, { forceRefetch: true });
	}

	policy(args: ScopedArgs) {
		return this.api.endpoints.agentPolicyGet.useQuery(args);
	}

	get installSkill() {
		return this.api.endpoints.agentSkillInstall.useMutation();
	}

	get uninstallSkill() {
		return this.api.endpoints.agentSkillUninstall.useMutation();
	}

	get setPolicy() {
		return this.api.endpoints.agentPolicySet.useMutation();
	}
}

function injectEndpoints(backendApi: BackendApi) {
	return backendApi.injectEndpoints({
		endpoints: (build) => ({
			agentsStatus: build.query<AgentsStatus, { projectId?: string }>({
				extraOptions: { command: "agents_status" },
				query: (args) => args,
				providesTags: [providesList(ReduxTag.AgentsStatus)],
			}),
			agentPolicyGet: build.query<PolicyState, ScopedArgs>({
				extraOptions: { command: "agent_policy_get" },
				query: (args) => args,
				providesTags: [providesList(ReduxTag.AgentPolicy)],
			}),
			agentSkillInstall: build.mutation<AgentsStatus, FrameworkArgs>({
				extraOptions: { command: "agent_skill_install" },
				query: (args) => args,
				invalidatesTags: () => [
					invalidatesList(ReduxTag.AgentsStatus),
					invalidatesList(ReduxTag.AgentPolicy),
				],
			}),
			agentSkillUninstall: build.mutation<
				AgentsStatus,
				FrameworkArgs & { removeInstructions?: boolean }
			>({
				extraOptions: { command: "agent_skill_uninstall" },
				query: (args) => args,
				invalidatesTags: () => [
					invalidatesList(ReduxTag.AgentsStatus),
					invalidatesList(ReduxTag.AgentPolicy),
				],
			}),
			agentPolicySet: build.mutation<PolicyState, ScopedArgs & { options: PolicyOptions }>({
				extraOptions: { command: "agent_policy_set" },
				query: (args) => args,
				invalidatesTags: () => [
					invalidatesList(ReduxTag.AgentPolicy),
					invalidatesList(ReduxTag.AgentsStatus),
				],
			}),
		}),
	});
}
