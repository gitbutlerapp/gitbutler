<script lang="ts">
	import BranchIntegrationGraph from "$components/branch/BranchIntegrationGraph.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { buildCurrentStateDisplayRows } from "$lib/upstream/branchIntegrationCurrentStateDisplay";
	import {
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
	import { Button, Icon, ModalFooter, RadioButton, TestId, Badge, chipToasts } from "@gitbutler/ui";
	import type { BranchIntegrationStrategy } from "@gitbutler/but-sdk";
	import type { IconName } from "@gitbutler/ui";

	type Props = {
		projectId: string;
		branchRef: string;
		branchName: string;
		closeModal: () => void;
	};

	let { projectId, branchRef, branchName, closeModal }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const DEFAULT_TEMPLATE: BranchIntegrationStrategy = "pullRebase";
	const integrationTemplates: Array<{
		id: BranchIntegrationStrategy;
		label: string;
		icon: IconName;
		description: string;
		recommended?: boolean;
	}> = [
		{
			id: "pullRebase",
			label: "Pull rebase",
			icon: "branch-top-up-arrow",
			description: "Rebuilds the branch with remote commits first, then your local commits.",
			recommended: true,
		},
		{
			id: "smartSquash",
			label: "Smart squash",
			icon: "branch-double-commit",
			description: "Merges matching commits (by Change ID), pull-rebases the rest.",
		},
		{
			id: "merge",
			label: "Merge",
			icon: "branch-merge",
			description: "Keeps local history and merges in the remote tip.",
		},
		{
			id: "pickRemote",
			label: "Pick remote",
			icon: "cherry-pick",
			description: "Rebuilds the branch from remote commits only.",
		},
	];
	const VISIBLE_TEMPLATE_COUNT = 2;
	let selectedTemplate = $state<BranchIntegrationStrategy>(DEFAULT_TEMPLATE);
	const initialBranchIntegration = $derived(
		stackService.initialBranchIntegration(projectId, branchRef, selectedTemplate),
	);
	const [applyBranchIntegration] = stackService.applyBranchIntegration;

	let preparedIntegration = $state<{
		mergeBase: string;
		firstLocalNotIntegrated: string | null;
		steps: IntegrationStepDraft[];
	}>();
	let previewRows = $state<IntegrationGraphRow[] | null>(null);
	let previewError = $state<string | null>(null);
	let applying = $state(false);
	let showAllStrategies = $state(false);
	let showIntegratedLocalCommits = $state(false);
	let templateSelectionVersion = 0;
	let initializedRequestId: string | undefined;

	const visibleTemplates = $derived(
		showAllStrategies
			? integrationTemplates
			: integrationTemplates.slice(0, VISIBLE_TEMPLATE_COUNT),
	);
	const hiddenTemplates = $derived(integrationTemplates.slice(VISIBLE_TEMPLATE_COUNT));

	function formatError(error: unknown): string {
		return error instanceof Error ? error.message : JSON.stringify(error);
	}

	async function previewIntegrationWithSteps(
		mergeBase: string,
		firstLocalNotIntegrated: string | null,
		steps: IntegrationStepDraft[],
		expectedTemplateSelectionVersion?: number,
	) {
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
		} catch (error) {
			if (
				expectedTemplateSelectionVersion !== undefined &&
				expectedTemplateSelectionVersion !== templateSelectionVersion
			) {
				return;
			}
			previewRows = null;
			previewError = formatError(error);
		}
	}

	function selectTemplate(template: BranchIntegrationStrategy) {
		if (template === selectedTemplate) return;

		selectedTemplate = template;
		templateSelectionVersion++;
		preparedIntegration = undefined;
		previewError = null;
	}

	async function applyIntegration() {
		if (!preparedIntegration) return;

		applying = true;
		previewError = null;
		try {
			const integration = buildInteractiveIntegration(preparedIntegration);
			await applyBranchIntegration({
				projectId,
				branchRef,
				integration,
				dryRun: false,
			});
			chipToasts.success(`Successfully updated "${branchName}".`);
			closeModal();
		} catch (error) {
			previewError = formatError(error);
		} finally {
			applying = false;
		}
	}

	$effect(() => {
		// Nothing to wipe while the query is not fulfilled: the prepared plan
		// starts unset, `selectTemplate` clears it when the key changes, and
		// wiping here on a background refetch (any workspace invalidation
		// lands one) flips Apply disabled for a round trip — the browser then
		// swallows a click on it without dispatching any event.
		const query = initialBranchIntegration.result;
		if (query.status !== "fulfilled") return;

		const initial = query.data;
		if (!initial || query.requestId === initializedRequestId) return;
		initializedRequestId = query.requestId;

		const nextStepDrafts = buildIntegrationStepDrafts(initial.integration);
		const version = ++templateSelectionVersion;
		preparedIntegration = {
			mergeBase: initial.integration.mergeBase,
			firstLocalNotIntegrated: initial.integration.firstLocalNotIntegrated,
			steps: nextStepDrafts,
		};
		previewRows = null;
		previewError = null;
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

<ReduxResult {projectId} result={initialBranchIntegration.result}>
	{#snippet children(initialIntegration)}
		{@const currentRows = buildCurrentStateGraphRows(initialIntegration)}
		{@const currentDisplayRows = buildCurrentStateDisplayRows({
			initialIntegration,
			currentRows,
			showIntegratedLocalCommits,
		})}
		<div class="branch-integration">
			<p class="text-13 text-body clr-text-2">
				This branch and its remote have diverged.
				<br />
				Pick an integration strategy below to combine them.
			</p>

			<div
				class="strategy-cards"
				class:strategy-cards_expanded={showAllStrategies}
				role="radiogroup"
				aria-label="Integration strategy selection"
				data-testid="branch-integration-strategies"
			>
				{#each visibleTemplates as template (template.id)}
					<label
						for={`strategy-${template.id}`}
						class="strategy-card"
						class:strategy-card_selected={selectedTemplate === template.id}
						data-testid={`branch-integration-template-${template.id}`}
					>
						<div class="strategy-card__header">
							<div class="strategy-card__icon">
								<Icon size={24} name={template.icon} />
							</div>
							<RadioButton
								checked={selectedTemplate === template.id}
								name="integration-strategy"
								id={`strategy-${template.id}`}
								onchange={() => selectTemplate(template.id)}
							/>
						</div>
						<h3 class="text-13 text-bold strategy-card__title">
							{template.label}
							{#if template.recommended}
								<span class="op-40">(Recommended)</span>
							{/if}
						</h3>
						<p class="text-12 text-body strategy-card__caption">
							{template.description}
						</p>
					</label>
				{/each}

				{#if !showAllStrategies}
					<button
						type="button"
						class="strategy-card strategy-card_more"
						data-testid="branch-integration-show-more-strategies"
						onclick={() => (showAllStrategies = true)}
					>
						<div class="strategy-card__more-icons">
							{#each hiddenTemplates as template (template.id)}
								<Icon size={20} name={template.icon} />
							{/each}
						</div>
						<span class="text-12 clr-text-2">
							{hiddenTemplates.map((t) => t.label.toLowerCase()).join(", ")}
						</span>
						<span class="text-12 underline-dotted strategy-card__more-link">
							Show {hiddenTemplates.length} more
						</span>
					</button>
				{/if}
			</div>

			<div class="modal-divider"></div>

			<div class="branch-integration__sections">
				<div class="branch-integration__section">
					<div class="branch-integration__section-header">
						<Badge style="gray" kind="soft" size="tag">CURRENT STATE</Badge>
						<div class="section-arrow">
							<div class="section-arrow__line"></div>
						</div>
					</div>

					<section class="branch-integration__graph" data-testid="branch-integration-current-state">
						<BranchIntegrationGraph
							isPreview={false}
							rows={currentDisplayRows}
							testId="branch-integration-current-state-row"
							{showIntegratedLocalCommits}
							toggleIntegratedLocalCommits={() =>
								(showIntegratedLocalCommits = !showIntegratedLocalCommits)}
						/>
					</section>
				</div>

				<div class="branch-integration__section">
					<div class="branch-integration__section-header">
						<Badge style="gray" size="tag">OUTPUT BRANCH</Badge>
					</div>

					<section class="branch-integration__graph" data-testid="branch-integration-preview">
						{#if previewError}
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
		</div>

		<ModalFooter>
			<Button kind="outline" type="reset" onclick={closeModal}>Cancel</Button>
			<Button
				style="pop"
				type="button"
				testId={TestId.BranchIntegrationApplyButton}
				onclick={applyIntegration}
				disabled={!preparedIntegration || preparedIntegration.steps.length === 0 || applying}
				loading={applying}
			>
				Apply integration
			</Button>
		</ModalFooter>
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.branch-integration {
		display: flex;
		flex-direction: column;
		justify-content: start;
		padding: 0 16px 16px 16px;
		overflow: hidden;
		gap: 12px;
	}

	.modal-divider {
		width: calc(100% + 32px);
		height: 1px;
		margin-block: 8px;
		margin-left: -16px;
		background: var(--border-3);
	}

	.branch-integration__sections {
		display: flex;
		width: 100%;
		overflow: hidden;
		gap: 12px;
	}

	.branch-integration__section {
		display: flex;
		flex: 1;
		flex-direction: column;
		overflow: hidden;
		gap: 12px;
	}

	.branch-integration__graph {
		display: flex;
		flex-direction: column;
		/* Modal is capped at calc(100vh - 80px); subtract header, intro,
		   footer and paddings so the graph scrolls instead of clipping. */
		max-height: calc(100vh - 380px);
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
		background: var(--bg-1);
	}

	.branch-integration__section-header {
		display: flex;
	}

	.section-arrow {
		display: flex;
		position: relative;
		flex: 1;
		align-items: center;
		width: 100%;
		margin-left: 10px;
		color: var(--border-2);

		&::before {
			width: 6px;
			height: 6px;
			border-radius: 8px;
			background-color: currentColor;
			content: "";
		}

		&::after {
			width: 0;
			height: 0;
			border-top: 4px solid transparent;
			border-bottom: 4px solid transparent;
			border-left: 4px solid currentColor;
			content: "";
		}
	}

	.section-arrow__line {
		width: 100%;
		height: 1px;
		background-color: currentColor;
	}

	.branch-integration__empty,
	.branch-integration__error {
		padding: 16px 14px;
		color: var(--text-2);
		font-size: 12px;
	}

	.branch-integration__error {
		color: var(--text-warn);
	}

	/* STRATEGY CARDS */
	.strategy-cards {
		display: flex;

		/* Collapse double borders between adjacent cards. */
		& .strategy-card + .strategy-card {
			margin-left: -1px;
		}

		&.strategy-cards_expanded {
			display: grid;
			grid-template-columns: repeat(2, 1fr);

			/* Collapse borders in the 2x2 grid: second column overlaps
			   left, second row overlaps top. */
			& .strategy-card:nth-child(2n) {
				margin-left: -1px;
			}
			& .strategy-card:nth-child(2n + 1) {
				margin-left: 0;
			}
			& .strategy-card:nth-child(n + 3) {
				margin-top: -1px;
			}

			/* Round the outer corners of the 2x2 grid. */
			& .strategy-card {
				border-radius: 0;
			}
			& .strategy-card:nth-child(1) {
				border-top-left-radius: var(--radius-m);
			}
			& .strategy-card:nth-child(2) {
				border-top-right-radius: var(--radius-m);
			}
			& .strategy-card:nth-child(3) {
				border-bottom-left-radius: var(--radius-m);
			}
			& .strategy-card:nth-child(4) {
				border-bottom-right-radius: var(--radius-m);
			}
		}
	}

	.strategy-card {
		--btn-bg: var(--btn-gray-outline-bg);
		--btn-bg-opacity: 0%;
		display: flex;
		position: relative;
		flex-direction: column;
		padding: 16px;
		gap: 6px;
		border: 1px solid var(--border-2);
		background: color-mix(
			in srgb,
			var(--btn-bg, transparent),
			transparent calc(100% - var(--btn-bg-opacity))
		);
		text-align: left;
		cursor: pointer;
		transition:
			border-color var(--transition-fast),
			background-color var(--transition-fast);

		&:first-child {
			border-top-left-radius: var(--radius-m);
			border-bottom-left-radius: var(--radius-m);
		}

		&:last-child {
			border-top-right-radius: var(--radius-m);
			border-bottom-right-radius: var(--radius-m);
		}

		&:not(.strategy-card_selected):hover {
			z-index: 1;
			--btn-bg-opacity: 12%;
			border-color: var(--border-1);
		}

		&.strategy-card_selected {
			/* Sits above neighbours so the highlighted border isn't
			   covered by the overlapping (collapsed) borders. */
			z-index: 2;
			border-color: var(--fill-pop-bg);
			background: color-mix(in srgb, var(--fill-pop-bg) 8%, transparent);
		}
	}

	.strategy-card__header {
		display: flex;
		justify-content: space-between;
		color: var(--text-2);
	}

	.strategy-card__title {
		margin-top: 4px;
	}

	.strategy-card__caption {
		color: var(--text-2);
	}

	.strategy-card_more {
		justify-content: flex-end;
		min-width: 160px;
		background-color: var(--bg-mute);
	}

	.strategy-card__more-icons {
		display: flex;
		margin-bottom: 4px;
		gap: 8px;
		color: var(--text-3);
	}

	.strategy-card__more-link {
		color: var(--text-2);
	}
</style>
