<script lang="ts">
	import BranchIntegrationGraph from "$components/branch/BranchIntegrationGraph.svelte";
	import BranchIntegrationSteps from "$components/branch/BranchIntegrationSteps.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { buildCurrentStateDisplayRows } from "$lib/upstream/branchIntegrationCurrentStateDisplay";
	import {
		buildCommitPickerOptions,
		buildIntegrationStepDrafts,
		buildInteractiveIntegration,
		type IntegrationStepDraft,
	} from "$lib/upstream/branchIntegrationEditor";
	import {
		buildCurrentStateGraphRows,
		buildNextStateGraphRows,
		type IntegrationGraphRow,
	} from "$lib/upstream/branchIntegrationView";
	import { inject } from "@gitbutler/core/context";
	import { Button, Modal, ModalFooter, SegmentControl, TestId } from "@gitbutler/ui";
	import type { BranchIntegrationStrategy } from "@gitbutler/but-sdk";

	type Props = {
		modalRef: Modal | undefined;
		projectId: string;
		branchName: string;
		branchRef: string;
	};

	let { modalRef = $bindable(), projectId, branchName, branchRef }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const DEFAULT_TEMPLATE: BranchIntegrationStrategy = "pullRebase";
	const integrationTemplates: Array<{ id: BranchIntegrationStrategy; label: string }> = [
		{ id: "pullRebase", label: "Pull rebase" },
		{ id: "smartSquash", label: "Smart squash" },
		{ id: "merge", label: "Merge" },
		{ id: "pickRemote", label: "Pick remote" },
	];
	const integrationTemplateDescriptions: Record<BranchIntegrationStrategy, string> = {
		pullRebase:
			"Rebuilds the branch picking first the commits on the remote, and then the commits on the local branch.",
		smartSquash:
			"Tries to fold matching remote work into related local commits. This is done through matching Change IDs, and falling back to pull-rebase ordering otherwise.",
		merge: "Keeps your local history and merges the remote tip into it.",
		pickRemote: "Rebuilds the branch picking only the commits on the remote.",
	};
	let selectedTemplate = $state<BranchIntegrationStrategy>(DEFAULT_TEMPLATE);
	const initialBranchIntegration = $derived(
		stackService.initialBranchIntegration(projectId, branchRef, selectedTemplate),
	);
	const [applyBranchIntegration, integrationMutation] = stackService.applyBranchIntegration;

	let stepDrafts = $state<IntegrationStepDraft[]>([]);
	let previewRows = $state<IntegrationGraphRow[] | null>(null);
	let previewError = $state<string | null>(null);
	let previewEmpty = $state(true);
	let activeAction = $state<"preview" | "apply" | null>(null);
	let initializedForOpen = $state(false);
	let showSteps = $state(false);
	let showIntegratedLocalCommits = $state(false);
	let templateSelectionVersion = 0;

	function closeModal() {
		modalRef?.close();
	}

	function formatError(error: unknown): string {
		return error instanceof Error ? error.message : JSON.stringify(error);
	}

	async function previewIntegrationWithSteps(
		mergeBase: string,
		firstLocalNotIntegrated: string | null,
		steps: IntegrationStepDraft[],
		expectedTemplateSelectionVersion: number | undefined = undefined,
	) {
		activeAction = "preview";
		previewError = null;
		try {
			const integration = buildInteractiveIntegration({
				mergeBase,
				firstLocalNotIntegrated,
				steps,
			});
			const result = await applyBranchIntegration({
				projectId,
				branchRef,
				integration,
				dryRun: true,
			});
			const nextPreviewRows = buildNextStateGraphRows({
				workspace: result.workspace,
				branchRef,
			});
			if (
				expectedTemplateSelectionVersion !== undefined &&
				expectedTemplateSelectionVersion !== templateSelectionVersion
			) {
				return;
			}
			previewRows = nextPreviewRows;
			previewEmpty = false;
		} catch (error) {
			if (
				expectedTemplateSelectionVersion !== undefined &&
				expectedTemplateSelectionVersion !== templateSelectionVersion
			) {
				return;
			}
			previewRows = null;
			previewError = formatError(error);
			previewEmpty = false;
		} finally {
			if (
				expectedTemplateSelectionVersion === undefined ||
				expectedTemplateSelectionVersion === templateSelectionVersion
			) {
				activeAction = null;
			}
		}
	}

	async function previewIntegration(mergeBase: string, firstLocalNotIntegrated: string | null) {
		await previewIntegrationWithSteps(mergeBase, firstLocalNotIntegrated, stepDrafts);
	}

	async function selectTemplate(template: BranchIntegrationStrategy) {
		if (template === selectedTemplate) return;

		selectedTemplate = template;
		const version = ++templateSelectionVersion;
		previewRows = null;
		previewError = null;
		previewEmpty = true;

		try {
			const initial = await stackService.fetchInitialBranchIntegration(
				projectId,
				branchRef,
				template,
			);
			if (version !== templateSelectionVersion) return;

			const nextStepDrafts = buildIntegrationStepDrafts(initial.integration);
			stepDrafts = nextStepDrafts;
			if (nextStepDrafts.length > 0) {
				await previewIntegrationWithSteps(
					initial.integration.mergeBase,
					initial.integration.firstLocalNotIntegrated,
					nextStepDrafts,
					version,
				);
			}
		} catch (error) {
			if (version !== templateSelectionVersion) return;
			previewRows = null;
			previewError = formatError(error);
			previewEmpty = false;
		}
	}

	async function applyIntegration(mergeBase: string, firstLocalNotIntegrated: string | null) {
		activeAction = "apply";
		previewError = null;
		try {
			const integration = buildInteractiveIntegration({
				mergeBase,
				firstLocalNotIntegrated,
				steps: stepDrafts,
			});
			await applyBranchIntegration({
				projectId,
				branchRef,
				integration,
				dryRun: false,
			});
			closeModal();
		} catch (error) {
			previewError = formatError(error);
		} finally {
			activeAction = null;
		}
	}

	$effect(() => {
		const modalOpen = modalRef?.imports.open ?? false;
		if (!modalOpen) {
			selectedTemplate = DEFAULT_TEMPLATE;
			templateSelectionVersion++;
			initializedForOpen = false;
			showSteps = false;
			showIntegratedLocalCommits = false;
			previewRows = null;
			previewError = null;
			previewEmpty = true;
		}
	});

	$effect(() => {
		const modalOpen = modalRef?.imports.open ?? false;
		const initial = initialBranchIntegration.response;
		if (!modalOpen || !initial || initializedForOpen) return;

		const nextStepDrafts = buildIntegrationStepDrafts(initial.integration);
		const version = templateSelectionVersion;
		stepDrafts = nextStepDrafts;
		previewRows = null;
		previewError = null;
		previewEmpty = true;
		initializedForOpen = true;
		if (nextStepDrafts.length > 0) {
			void previewIntegrationWithSteps(
				initial.integration.mergeBase,
				initial.integration.firstLocalNotIntegrated,
				nextStepDrafts,
				version,
			);
		}
	});
</script>

<Modal
	bind:this={modalRef}
	title={`Update ${branchName}`}
	noPadding
	width="full-screen"
	testId={TestId.BranchIntegrationModal}
>
	<ReduxResult {projectId} result={initialBranchIntegration.result}>
		{#snippet children(initialIntegration)}
			{@const commitOptions = buildCommitPickerOptions(initialIntegration)}
			{@const currentRows = buildCurrentStateGraphRows(initialIntegration)}
			{@const currentDisplayRows = buildCurrentStateDisplayRows({
				initialIntegration,
				currentRows,
				showIntegratedLocalCommits,
			})}
			<div class="branch-integration">
				<div class="branch-integration__intro">
					<p class="text-13">The local branch and its remote counterpart have diverged.</p>
					<p class="text-13">
						You can review the divergence between them, decide if and how to integrate the changes
						into your local branch, preview and apply the changes.
					</p>
				</div>

				<div class="branch-integration__sections">
					<section
						class="branch-integration__section"
						data-testid="branch-integration-current-state"
					>
						<div class="branch-integration__section-header">
							<h3 class="text-13 text-semibold">Current state</h3>
						</div>
						<BranchIntegrationGraph
							isPreview={false}
							rows={currentDisplayRows}
							testId="branch-integration-current-state-row"
							{showIntegratedLocalCommits}
							toggleIntegratedLocalCommits={() =>
								(showIntegratedLocalCommits = !showIntegratedLocalCommits)}
						/>
					</section>

					<section class="branch-integration__section" data-testid="branch-integration-steps">
						<div class="branch-integration__section-header">
							<h3 class="text-13 text-semibold">Integration strategy</h3>
							<div class="branch-integration__section-actions">
								<SegmentControl
									size="small"
									selected={selectedTemplate}
									onselect={(id) => selectTemplate(id as BranchIntegrationStrategy)}
								>
									{#each integrationTemplates as template (template.id)}
										<SegmentControl.Item
											id={template.id}
											testId={`branch-integration-template-${template.id}`}
										>
											{template.label}
										</SegmentControl.Item>
									{/each}
								</SegmentControl>
								<Button
									kind="outline"
									size="tag"
									icon={showSteps ? "chevron-up" : "chevron-down"}
									testId={TestId.BranchIntegrationToggleStepsButton}
									onclick={() => (showSteps = !showSteps)}
								>
									{showSteps ? "Hide steps" : "Show steps"}
								</Button>
							</div>
						</div>
						{#if showSteps}
							<BranchIntegrationSteps bind:stepDrafts {commitOptions} />
						{:else}
							<div class="branch-integration__collapsed-note text-12 clr-text-2">
								Using the selected strategy steps. Expand to inspect or edit them.
								<ul class="branch-integration__strategy-descriptions">
									{#each integrationTemplates as template (template.id)}
										<li>
											<span class="text-semibold">{template.label}:</span>
											{integrationTemplateDescriptions[template.id]}
										</li>
									{/each}
								</ul>
							</div>
						{/if}
					</section>

					<section class="branch-integration__section" data-testid="branch-integration-preview">
						<div class="branch-integration__section-header">
							<h3 class="text-13 text-semibold">Preview</h3>
						</div>
						{#if previewEmpty}
							<div class="branch-integration__empty" data-testid="branch-integration-empty-state">
								Run preview to inspect the resulting branch shape.
							</div>
						{:else if previewError}
							<div class="branch-integration__error" data-testid="branch-integration-error">
								{previewError}
							</div>
						{:else if previewRows === null}
							<div class="branch-integration__empty" data-testid="branch-integration-empty-state">
								Preview produced no branch segment for this ref.
							</div>
						{:else if previewRows.length === 0}
							<div class="branch-integration__empty" data-testid="branch-integration-empty-state">
								The resulting branch would be empty.
							</div>
						{:else}
							<BranchIntegrationGraph
								isPreview={true}
								rows={previewRows}
								testId="branch-integration-preview-row"
							/>
						{/if}
					</section>
				</div>
			</div>

			<ModalFooter>
				<Button kind="outline" type="reset" onclick={closeModal}>Cancel</Button>
				<Button
					kind="outline"
					type="button"
					testId={TestId.BranchIntegrationPreviewButton}
					onclick={() =>
						previewIntegration(
							initialIntegration.integration.mergeBase,
							initialIntegration.integration.firstLocalNotIntegrated,
						)}
					disabled={stepDrafts.length === 0 || integrationMutation.current.isLoading}
					loading={integrationMutation.current.isLoading && activeAction === "preview"}
				>
					Preview
				</Button>
				<Button
					style="pop"
					type="button"
					testId={TestId.BranchIntegrationApplyButton}
					onclick={() =>
						applyIntegration(
							initialIntegration.integration.mergeBase,
							initialIntegration.integration.firstLocalNotIntegrated,
						)}
					disabled={stepDrafts.length === 0 || integrationMutation.current.isLoading}
					loading={integrationMutation.current.isLoading && activeAction === "apply"}
				>
					Apply integration
				</Button>
			</ModalFooter>
		{/snippet}
	</ReduxResult>
</Modal>

<style lang="postcss">
	.branch-integration {
		display: flex;
		flex-direction: column;
		justify-content: start;
		height: 70vh;
		padding: 0 16px 16px 16px;
		gap: 12px;
	}

	.branch-integration__intro {
		display: flex;
		flex-direction: column;
		padding-top: 16px;
		gap: 4px;
	}

	.branch-integration__sections {
		display: flex;
		justify-content: stretch;
		width: 100%;
		height: 100%;
		overflow: hidden;
		gap: 12px;
	}

	.branch-integration__section {
		box-sizing: border-box;
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-ml);
		background: var(--bg-1);
	}

	.branch-integration__section-header {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: space-between;
		padding: 12px 14px;
		gap: 10px;
		border-bottom: 1px solid var(--border-2);
	}

	.branch-integration__section-actions {
		display: flex;
		flex-direction: column;
		align-items: center;
		min-width: 0;
		gap: 8px;
	}

	.branch-integration__empty,
	.branch-integration__error {
		padding: 16px 14px;
		color: var(--text-2);
		font-size: 12px;
	}

	.branch-integration__collapsed-note {
		padding: 16px 14px;
	}

	.branch-integration__strategy-descriptions {
		margin: 8px 0 0;
	}

	.branch-integration__strategy-descriptions li + li {
		margin-top: 8px;
	}

	.branch-integration__error {
		color: var(--text-warn);
	}
</style>
