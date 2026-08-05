<script lang="ts">
	import AgentSkillRow from "$components/settings/AgentSkillRow.svelte";
	import { AGENTS_SERVICE } from "$lib/agents/agentsService";
	import { inject } from "@gitbutler/core/context";
	import { Button, CardGroup } from "@gitbutler/ui";
	import type { AgentFramework, FrameworkScopeState, SkillScope } from "@gitbutler/but-sdk";

	type Props = {
		scope: SkillScope;
		projectId?: string;
	};

	const { scope, projectId }: Props = $props();

	const agentsService = inject(AGENTS_SERVICE);
	const status = $derived(agentsService.status({ projectId }));

	let showAll = $state(false);

	/** Only frameworks that can install at this scope; some agents are global-only. */
	const available = $derived(
		(status.response?.frameworks ?? []).filter((framework: AgentFramework) =>
			framework.scopes.some((entry: FrameworkScopeState) => entry.scope === scope),
		),
	);

	const detected = $derived(
		available.filter((framework: AgentFramework) =>
			scope === "global" ? framework.detectedGlobally : framework.detectedInRepo,
		),
	);

	/**
	 * An installed skill keeps its row visible even when the agent is no longer
	 * detected, so a skill can always be uninstalled from where it was installed.
	 */
	const installedButUndetected = $derived(
		available.filter(
			(framework: AgentFramework) =>
				!detected.includes(framework) &&
				framework.scopes.some(
					(entry: FrameworkScopeState) => entry.scope === scope && entry.installed,
				),
		),
	);

	const alwaysShown = $derived([...detected, ...installedButUndetected]);
	const rest = $derived(
		available.filter((framework: AgentFramework) => !alwaysShown.includes(framework)),
	);
	const shown = $derived(showAll ? [...alwaysShown, ...rest] : alwaysShown);
</script>

<CardGroup>
	{#if status.response && alwaysShown.length === 0 && !showAll}
		<CardGroup.Item>
			{#snippet title()}No coding agents detected{/snippet}
			{#snippet caption()}
				GitButler looks for each agent's config to decide what to show here. Choose "Show all
				agents" to install a skill anyway.
			{/snippet}
		</CardGroup.Item>
	{/if}

	{#each shown as framework (framework.id)}
		<AgentSkillRow {framework} {scope} {projectId} />
	{/each}
</CardGroup>

{#if rest.length > 0}
	<div class="flex justify-center m-t-12">
		<Button kind="ghost" onclick={() => (showAll = !showAll)}>
			{showAll ? "Show detected agents only" : `Show all agents (${rest.length} more)`}
		</Button>
	</div>
{/if}
