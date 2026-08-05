<script lang="ts">
	import CliInstallCard from "$components/settings/CliInstallCard.svelte";
	import { AGENTS_SERVICE } from "$lib/agents/agentsService";
	import { CLI_MANAGER } from "$lib/config/cli";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { inject } from "@gitbutler/core/context";
	import { Button, Checkbox, ModalFooter, ModalHeader, chipToasts } from "@gitbutler/ui";
	import type { AgentSetupModalState } from "$lib/state/uiState.svelte";
	import type { AgentFramework, SkillScope } from "@gitbutler/but-sdk";

	type Props = {
		data: AgentSetupModalState;
		close: () => void;
	};

	const { data, close }: Props = $props();

	const agentsService = inject(AGENTS_SERVICE);
	const settingsService = inject(SETTINGS_SERVICE);
	const cliManager = inject(CLI_MANAGER);

	const cliState = cliManager.state();
	const status = $derived(agentsService.status({ projectId: data.projectId }));
	const [installSkill, installing] = agentsService.installSkill;

	type Step = "cli" | "agents" | "scope" | "review";

	let step = $state<Step>("cli");
	let selectedIds = $state<string[]>([]);
	let scope = $state<SkillScope>("global");
	let seeded = false;

	const cliReady = $derived(!!cliState.response?.installed || !!cliState.response?.blockedReason);
	const frameworks = $derived(status.response?.frameworks ?? []);

	// Pre-select the agents we can see the user already works with, matching
	// what `but agent setup` does. Seeded once so a background refetch cannot
	// undo the user's own ticks.
	$effect(() => {
		if (seeded || frameworks.length === 0) return;
		seeded = true;
		selectedIds = frameworks
			.filter((framework: AgentFramework) => framework.detectedGlobally || framework.detectedInRepo)
			.map((framework: AgentFramework) => framework.id);
		if (cliReady && step === "cli") step = "agents";
	});

	const eligible = $derived(
		frameworks.filter((framework: AgentFramework) =>
			framework.scopes.some((entry) => entry.scope === scope),
		),
	);
	const chosen = $derived(
		eligible.filter((framework: AgentFramework) => selectedIds.includes(framework.id)),
	);

	function toggle(id: string) {
		selectedIds = selectedIds.includes(id)
			? selectedIds.filter((entry) => entry !== id)
			: [...selectedIds, id];
	}

	async function apply() {
		for (const framework of chosen) {
			await installSkill({ frameworkId: framework.id, scope, projectId: data.projectId });
		}
		// Completing setup answers the prompt, so it should not be asked again
		// even if the user only set up one scope.
		await settingsService.updateAgents({ skillsPromptDismissed: true });
		chipToasts.success(
			`Installed the GitButler skill for ${chosen.length} agent${chosen.length === 1 ? "" : "s"}`,
		);
		close();
	}
</script>

<ModalHeader>Set up GitButler for your coding agents</ModalHeader>

<div class="wizard">
	<div class="step-section">
		<div class="step-line" class:step-line-default={step !== "cli"}></div>
		<div class="step-section__content">
			<h4 class="text-14 text-semibold">1. Install the <code>but</code> CLI</h4>
			<p class="text-12 clr-text-2">
				Your agent runs <code>but</code> to branch, commit, and open pull requests.
			</p>
			{#if step === "cli"}
				<CliInstallCard />
				<div class="flex justify-end m-t-8">
					<Button kind="outline" onclick={() => (step = "agents")}>
						{cliReady ? "Continue" : "Skip for now"}
					</Button>
				</div>
			{/if}
		</div>
	</div>

	{#if step !== "cli"}
		<div class="step-section">
			<div class="step-line" class:step-line-default={step !== "agents"}></div>
			<div class="step-section__content">
				<h4 class="text-14 text-semibold">2. Choose your agents</h4>
				{#if step === "agents"}
					<p class="text-12 clr-text-2">Detected agents are already ticked.</p>
					<div class="flex flex-col gap-8 m-t-8">
						{#each frameworks as framework (framework.id)}
							<div class="flex gap-8 items-center">
								<Checkbox
									id="agent-{framework.id}"
									checked={selectedIds.includes(framework.id)}
									onchange={() => toggle(framework.id)}
								/>
								<label for="agent-{framework.id}" class="text-13">
									{framework.name}
									{#if framework.detectedGlobally || framework.detectedInRepo}
										<span class="clr-text-2">— detected</span>
									{/if}
								</label>
							</div>
						{/each}
					</div>
					<div class="flex justify-end gap-8 m-t-8">
						<Button kind="outline" onclick={() => (step = "cli")}>Back</Button>
						<Button
							style="pop"
							disabled={selectedIds.length === 0}
							onclick={() => (step = data.projectId ? "scope" : "review")}>Continue</Button
						>
					</div>
				{/if}
			</div>
		</div>
	{/if}

	{#if data.projectId && (step === "scope" || step === "review")}
		<div class="step-section">
			<div class="step-line" class:step-line-default={step !== "scope"}></div>
			<div class="step-section__content">
				<h4 class="text-14 text-semibold">3. Where should this apply?</h4>
				{#if step === "scope"}
					<div class="flex flex-col gap-8 m-t-8">
						<label class="text-13 flex gap-8 items-center">
							<input type="radio" bind:group={scope} value="global" />
							All my projects
						</label>
						<label class="text-13 flex gap-8 items-center">
							<input type="radio" bind:group={scope} value="repository" />
							Just this project
						</label>
					</div>
					<div class="flex justify-end gap-8 m-t-8">
						<Button kind="outline" onclick={() => (step = "agents")}>Back</Button>
						<Button style="pop" onclick={() => (step = "review")}>Continue</Button>
					</div>
				{/if}
			</div>
		</div>
	{/if}

	{#if step === "review"}
		<div class="step-section">
			<div class="step-line step-line-last"></div>
			<div class="step-section__content">
				<h4 class="text-14 text-semibold">Review</h4>
				<p class="text-12 clr-text-2">These files will be written:</p>
				<ul class="paths">
					{#each chosen as framework (framework.id)}
						{@const entry = framework.scopes.find((item) => item.scope === scope)}
						{#if entry}
							<li><code class="code-string">{entry.skillPath}</code></li>
						{/if}
					{/each}
				</ul>
				{#if chosen.length === 0}
					<p class="text-12 clr-text-2">None of the agents you picked can install at this scope.</p>
				{/if}
			</div>
		</div>
	{/if}
</div>

<ModalFooter>
	<Button kind="outline" onclick={close}>Cancel</Button>
	{#if step === "review"}
		<Button
			style="pop"
			disabled={chosen.length === 0}
			loading={installing.current.isLoading}
			onclick={apply}>Set up</Button
		>
	{/if}
</ModalFooter>

<style lang="postcss">
	.wizard {
		display: flex;
		flex-direction: column;
		padding: 16px;
		gap: 4px;
	}

	.step-section {
		display: flex;
		position: relative;
		gap: 12px;
	}

	.step-line {
		width: 2px;
		border-radius: 1px;
		background-color: var(--clr-border-3);
	}

	.step-line-default {
		background-color: var(--clr-theme-pop-element);
	}

	.step-line-last {
		background-color: var(--clr-theme-pop-element);
	}

	.step-section__content {
		display: flex;
		flex: 1;
		flex-direction: column;
		padding-bottom: 16px;
		gap: 4px;
	}

	.paths {
		display: flex;
		flex-direction: column;
		margin-top: 4px;
		gap: 4px;
	}
</style>
