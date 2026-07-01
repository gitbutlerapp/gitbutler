<script lang="ts">
	import {
		changeIntegrationStepDraftKind,
		type CommitPickerOption,
		createDefaultIntegrationStepDraft,
		type IntegrationStepDraft,
		reorderIntegrationStepDrafts,
		updateIntegrationStepDraftCommit,
		updateIntegrationStepDraftMessage,
	} from "$lib/upstream/branchIntegrationEditor";
	import { Button } from "@gitbutler/ui";

	type Props = {
		stepDrafts: IntegrationStepDraft[];
		commitOptions: CommitPickerOption[];
	};

	let { stepDrafts = $bindable(), commitOptions }: Props = $props();

	function displayCommitOption(option: CommitPickerOption): string {
		const refs = option.refs.length === 0 ? "" : ` (${option.refs.join(", ")})`;
		return `${option.id.slice(0, 7)}${refs} ${option.subject}`;
	}

	function moveStep(stepId: string, offset: -1 | 1) {
		const sourceIndex = stepDrafts.findIndex((step) => step.id === stepId);
		if (sourceIndex === -1) return;
		const destinationIndex = offset === 1 ? sourceIndex + 2 : sourceIndex - 1;
		if (destinationIndex < 0 || destinationIndex > stepDrafts.length) return;
		stepDrafts = reorderIntegrationStepDrafts({
			steps: stepDrafts,
			draggedStepId: stepId,
			destinationIndex,
		});
	}

	function deleteStep(stepId: string) {
		stepDrafts = stepDrafts.filter((step) => step.id !== stepId);
	}
</script>

<div class="branch-integration__actions">
	<Button
		kind="outline"
		size="tag"
		testId="branch-integration-clear-steps-button"
		disabled={stepDrafts.length === 0}
		onclick={() => (stepDrafts = [])}
		icon="bin"
	>
		Clear steps
	</Button>

	<Button
		kind="outline"
		size="tag"
		icon="plus"
		testId="branch-integration-add-step-button"
		disabled={commitOptions.length === 0}
		onclick={() => (stepDrafts = [...stepDrafts, createDefaultIntegrationStepDraft(commitOptions)])}
	>
		Add step
	</Button>
</div>

<div class="branch-integration__steps">
	{#if stepDrafts.length === 0}
		<div class="branch-integration__empty">
			No integration steps yet. Add a step to build the plan.
		</div>
	{:else}
		{#each stepDrafts as step, index (step.id)}
			<div
				class="branch-integration__step"
				data-testid="branch-integration-step"
				data-branch-integration-step-kind={step.kind}
				data-branch-integration-step-index={index}
			>
				<div class="branch-integration__step-toolbar">
					<span class="text-11 clr-text-2">Step {index + 1}</span>
					<div class="branch-integration__step-actions">
						<Button
							kind="outline"
							size="tag"
							icon="arrow-up"
							disabled={index === 0}
							onclick={() => moveStep(step.id, -1)}
						/>
						<Button
							kind="outline"
							size="tag"
							icon="arrow-down"
							disabled={index === stepDrafts.length - 1}
							onclick={() => moveStep(step.id, 1)}
						/>
						<Button kind="outline" size="tag" onclick={() => deleteStep(step.id)}>Delete</Button>
					</div>
				</div>

				<div class="branch-integration__step-fields">
					<label class="branch-integration__field">
						<select
							aria-label="Integration step type"
							value={step.kind}
							onchange={(event) =>
								(stepDrafts = stepDrafts.map((candidate) =>
									candidate.id === step.id
										? changeIntegrationStepDraftKind({
												step: candidate,
												kind: event.currentTarget.value as IntegrationStepDraft["kind"],
												commitOptions,
											})
										: candidate,
								))}
						>
							<option value="pick">Pick</option>
							<option value="merge">Merge</option>
							<option value="squash">Squash</option>
						</select>
					</label>

					{#if step.kind === "squash"}
						{#each step.commitIds as commitId, commitIndex}
							<label class="branch-integration__field">
								<select
									aria-label={`Squash commit ${commitIndex + 1}`}
									value={commitId}
									onchange={(event) =>
										(stepDrafts = stepDrafts.map((candidate) =>
											candidate.id === step.id
												? updateIntegrationStepDraftCommit({
														step: candidate,
														commitId: event.currentTarget.value,
														index: commitIndex,
														commitOptions,
													})
												: candidate,
										))}
								>
									{@render commitOptionsMarkup(
										commitOptions,
										step.commitIds.filter((_, index) => index !== commitIndex),
									)}
								</select>
							</label>
						{/each}
						<label class="branch-integration__field branch-integration__field--full">
							<textarea
								aria-label="Squash commit message"
								rows="3"
								value={step.message}
								oninput={(event) =>
									(stepDrafts = stepDrafts.map((candidate) =>
										candidate.id === step.id
											? updateIntegrationStepDraftMessage({
													step: candidate,
													message: event.currentTarget.value,
												})
											: candidate,
									))}
							></textarea>
						</label>
					{:else}
						<label class="branch-integration__field branch-integration__field--full">
							<select
								aria-label="Integration commit"
								value={step.commitId}
								onchange={(event) =>
									(stepDrafts = stepDrafts.map((candidate) =>
										candidate.id === step.id
											? updateIntegrationStepDraftCommit({
													step: candidate,
													commitId: event.currentTarget.value,
													commitOptions,
												})
											: candidate,
									))}
							>
								{@render commitOptionsMarkup(commitOptions)}
							</select>
						</label>
					{/if}
				</div>
			</div>
		{/each}
	{/if}
</div>

{#snippet commitOptionsMarkup(commitOptions: CommitPickerOption[], excludeCommitIds: string[] = [])}
	{@const groups = ["Local", "Upstream", "Shared"] as const}
	{@const filteredOptions = commitOptions.filter((option) => !excludeCommitIds.includes(option.id))}
	{#each groups as group}
		{@const options = filteredOptions.filter((option) => option.group === group)}
		{#if options.length > 0}
			<optgroup label={group}>
				{#each options as option (option.id)}
					<option value={option.id}>
						{displayCommitOption(option)}
					</option>
				{/each}
			</optgroup>
		{/if}
	{/each}
{/snippet}

<style lang="postcss">
	.branch-integration__actions {
		display: flex;
		flex-shrink: 0;
		justify-content: end;
		padding: 12px 14px;
		gap: 8px;
		border-bottom: 1px solid var(--border-2);
	}

	.branch-integration__steps {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-height: 0;
		overflow: auto;
	}

	.branch-integration__step {
		display: flex;
		flex-shrink: 0;
		flex-direction: column;
		padding: 12px 14px;
		gap: 10px;
		border-bottom: 1px solid var(--border-2);

		&:last-child {
			border-bottom: none;
		}
	}

	.branch-integration__step-toolbar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
	}

	.branch-integration__step-actions {
		display: flex;
		gap: 4px;
	}

	.branch-integration__step-fields {
		display: flex;
		gap: 10px;
	}

	.branch-integration__field {
		display: flex;
		flex-direction: column;
		gap: 6px;

		& select,
		& textarea {
			width: 100%;
			padding: 8px 10px;
			border: 1px solid var(--border-2);
			border-radius: var(--radius-m);
			background: var(--bg-2);
			color: var(--text-1);
		}
	}

	.branch-integration__field--full {
		grid-column: 1 / -1;
	}

	.branch-integration__empty {
		flex-shrink: 0;
		padding: 16px 14px;
		color: var(--text-2);
		font-size: 12px;
	}
</style>
