<script lang="ts">
	import BranchIntegrationModal from "$components/branch/BranchIntegrationModal.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	// import { persisted } from "@gitbutler/shared/persisted";
	import { Button, Modal, TestId } from "@gitbutler/ui";

	type IntegrationMode = "rebase" | "interactive";

	type Props = {
		projectId: string;
		stackId: string | undefined;
		branchName: string;
		branchRef: string;
	};

	const { projectId, stackId, branchName, branchRef }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const [integrateUpstreamCommits, integrating] = stackService.integrateUpstreamCommits;

	// const integrationMode = persisted<IntegrationMode>("rebase", "branchUpstreamIntegrationMode");

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

	// function getLabelForIntegrationMode(mode: IntegrationMode): string {
	// 	switch (mode) {
	// 		case "rebase":
	// 			return "Rebase";
	// 		case "interactive":
	// 			return "Configure integration…";
	// 	}
	// }
</script>

<BranchIntegrationModal bind:modalRef={integrationModal} {projectId} {branchName} {branchRef} />

<!-- {#snippet integrationRadioOption(mode: IntegrationMode, title: string, description: string)}
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
{/snippet} -->

<form
	class="upstream-integration-actions"
	onsubmit={(e) => {
		e.preventDefault();
		integrate("interactive");
	}}
>
	<!-- <div class="upstream-integration-actions__radio-container">
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
	</div> -->

	<Button
		type="submit"
		style="warning"
		disabled={integrating.current.isLoading}
		testId={TestId.UpstreamCommitsIntegrateButton}
	>
		integr8 don't h8
	</Button>
</form>

<style lang="postcss">
	.upstream-integration-actions {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}
</style>
