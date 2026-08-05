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
	const [updateSkills, updating] = agentsService.updateSkills;

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
				<code class="code-string skill-path">{scopeState.installed.path}</code>
			{:else}
				{framework.description}
			{/if}
		{/snippet}

		{#snippet actions()}
			<!--
				The shared actions column neither adds a gap nor stops itself
				shrinking, so three labelled buttons overflow the card. Only the
				action worth reading stays labelled; the rest are icon-only with
				tooltips.
			-->
			<div class="row-actions">
				{#if scopeState.installed}
					{#if !scopeState.installed.upToDate}
						<Button
							style="pop"
							icon="refresh"
							loading={updating.current.isLoading}
							onclick={() => updateSkills({ frameworkId: framework.id, scope, projectId })}
						>
							Update
						</Button>
					{/if}
					{#if scopeState.instructionPath}
						<Tooltip text="Customize workflow preferences">
							<Button kind="outline" icon="settings" onclick={() => policyModal?.show()} />
						</Tooltip>
					{/if}
					<Tooltip text="Uninstall this skill">
						<Button
							kind="outline"
							style="danger"
							icon="bin"
							loading={uninstalling.current.isLoading}
							onclick={() => uninstallSkill({ frameworkId: framework.id, scope, projectId })}
						/>
					</Tooltip>
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
			</div>
		{/snippet}
	</CardGroup.Item>

	<AgentWorkflowPolicyModal bind:this={policyModal} {scope} {projectId} />
{/if}

<style lang="postcss">
	.row-actions {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		gap: 6px;
	}

	/* A long path would otherwise set the content column's min width and
	   squeeze the actions off the card. */
	.skill-path {
		display: inline-block;
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		vertical-align: bottom;
		white-space: nowrap;
	}
</style>
