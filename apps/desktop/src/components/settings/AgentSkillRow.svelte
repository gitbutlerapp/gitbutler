<script lang="ts">
	import AgentWorkflowPolicyModal from "$components/settings/AgentWorkflowPolicyModal.svelte";
	import { AGENTS_SERVICE } from "$lib/agents/agentsService";
	import { inject } from "@gitbutler/core/context";
	import { Badge, Button, CardGroup, Tooltip } from "@gitbutler/ui";
	import type { AgentFramework, FrameworkScopeState, SkillScope } from "@gitbutler/but-sdk";

	type Props = {
		framework: AgentFramework;
		scope: SkillScope;
		projectId?: string;
	};

	const { framework, scope, projectId }: Props = $props();

	const agentsService = inject(AGENTS_SERVICE);
	// One mutation hook per row, so only the row being changed shows a spinner.
	const [installSkill, installing] = agentsService.installSkill;
	const [uninstallSkill, uninstalling] = agentsService.uninstallSkill;

	const scopeState = $derived(
		framework.scopes.find((entry: FrameworkScopeState) => entry.scope === scope),
	);
	const detected = $derived(
		scope === "global" ? framework.detectedGlobally : framework.detectedInRepo,
	);

	let policyModal = $state<AgentWorkflowPolicyModal>();
</script>

{#if scopeState}
	<CardGroup.Item>
		{#snippet title()}
			<span class="flex gap-6 items-center">
				{framework.name}
				{#if detected}
					<Badge style="pop">Detected</Badge>
				{/if}
				{#if scopeState.installed && !scopeState.installed.upToDate}
					<Tooltip text="Installed version {scopeState.installed.version} is out of date.">
						<Badge style="warning">Outdated</Badge>
					</Tooltip>
				{/if}
			</span>
		{/snippet}

		{#snippet caption()}
			{#if scopeState.installed}
				<code class="code-string">{scopeState.installed.path}</code>
			{:else}
				{framework.description}
			{/if}
		{/snippet}

		{#snippet actions()}
			{#if scopeState.installed}
				{#if scopeState.instructionPath}
					<Button kind="outline" icon="settings" onclick={() => policyModal?.show()}>
						Customize
					</Button>
				{/if}
				<Button
					kind="outline"
					style="danger"
					loading={uninstalling.current.isLoading}
					onclick={() => uninstallSkill({ frameworkId: framework.id, scope, projectId })}
				>
					Uninstall
				</Button>
			{:else}
				<Button
					style="pop"
					icon="plus"
					loading={installing.current.isLoading}
					onclick={() => installSkill({ frameworkId: framework.id, scope, projectId })}
				>
					Install
				</Button>
			{/if}
		{/snippet}
	</CardGroup.Item>

	<AgentWorkflowPolicyModal bind:this={policyModal} {scope} {projectId} />
{/if}
