<script lang="ts">
	import BranchIntegrationModal from "$components/branch/BranchIntegrationModal.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { persisted } from "@gitbutler/shared/persisted";
	import { Button, Modal, RadioButton, TestId } from "@gitbutler/ui";

	type IntegrationMode = "rebase" | "interactive";

	type Props = {
		projectId: string;
		stackId: string | undefined;
		branchName: string;
	};

	const { projectId, stackId, branchName }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const [integrateUpstreamCommits, integrating] = stackService.integrateUpstreamCommits;

	const integrationMode = persisted<IntegrationMode>("rebase", "branchUpstreamIntegrationMode");

	let integrationModal = $state<Modal>();

	function kickOffIntegration() {
		integrationModal?.show();
	}

	function handleRebaseIntegration() {
		if (!stackId) return;
		integrateUpstreamCommits({
			projectId,
			stackId,
			seriesName: branchName,
			integrationStrategy: { type: "rebase" },
		});
	}

	function integrate(mode: IntegrationMode) {
		switch (mode) {
			case "rebase":
				handleRebaseIntegration();
				break;
			case "interactive":
				kickOffIntegration();
				break;
		}
	}

	function getLabelForIntegrationMode(mode: IntegrationMode): string {
		switch (mode) {
			case "rebase":
				return "Rebase";
			case "interactive":
				return "Configure integration…";
		}
	}
</script>

<BranchIntegrationModal bind:modalRef={integrationModal} {projectId} {stackId} {branchName} />

{#snippet integrationRadioOption(mode: IntegrationMode, title: string, description: string)}
	<label class="integration-radio-option" class:selected={$integrationMode === mode}>
		<div class="integration-radio-content">
			<h4 class="text-13 text-semibold">{title}</h4>
			<p class="text-11 text-body clr-text-2">
				{description}
			</p>
		</div>
		<RadioButton
			class="integration-radio-option__radio"
			name="integrationMode"
			value={mode}
			checked={$integrationMode === mode}
			onchange={() => integrationMode.set(mode)}
		/>
	</label>
{/snippet}

<form
	class="upstream-integration-actions"
	onsubmit={(e) => {
		e.preventDefault();
		integrate($integrationMode);
	}}
>
	<div class="upstream-integration-actions__radio-container">
		{@render integrationRadioOption(
			"rebase",
			"Rebase upstream changes",
			"Place local-only changes on top, then the upstream changes. Similar to git pull --rebase.",
		)}
		{@render integrationRadioOption(
			"interactive",
			"Interactive integration",
			"Review and resolve any conflicts before completing the integration.",
		)}
	</div>

	<Button
		type="submit"
		style="warning"
		disabled={integrating.current.isLoading}
		testId={TestId.UpstreamCommitsIntegrateButton}
	>
		{getLabelForIntegrationMode($integrationMode)}
	</Button>
</form>

<style lang="postcss">
	.upstream-integration-actions {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	.upstream-integration-actions__radio-container {
		display: flex;
		flex-direction: column;
	}

	.integration-radio-option {
		display: flex;
		z-index: 0;
		position: relative;
		padding: 14px;
		gap: 20px;
		border: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
		cursor: pointer;

		&:not(.selected):hover {
			background: var(--hover-bg-1);
		}

		&:first-child {
			border-top-right-radius: var(--radius-m);
			border-top-left-radius: var(--radius-m);
		}
		&:last-child {
			margin-top: -1px;
			border-bottom-right-radius: var(--radius-m);
			border-bottom-left-radius: var(--radius-m);
		}

		&.selected {
			z-index: 1;
			border-color: var(--clr-theme-pop-element);
			background: var(--clr-theme-pop-bg);
		}
	}

	:global(.integration-radio-option__radio) {
		flex-shrink: 0;
	}

	.integration-radio-content {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: 6px;
	}
</style>
