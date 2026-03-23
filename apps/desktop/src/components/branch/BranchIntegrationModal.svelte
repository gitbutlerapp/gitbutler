<script lang="ts">
	import CommitTimelineNode from "$components/commit/CommitTimelineNode.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
	import {
		commitCreatedAtDate,
		extractUpstreamCommitId,
		isCommit,
		type Commit,
		type UpstreamCommit,
	} from "$lib/branches/v3";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import {
		canShiftStepDown,
		canShiftStepUp,
		getStepCommitInfo,
		pickLocalStep,
		pickUpstreamStep,
		shiftStepDown,
		shiftStepUp,
		splitStepAtCommit,
		squashStepInto,
		updateStepType,
	} from "$lib/upstream/integrationStepUtils";
	import { inject } from "@gitbutler/core/context";
	import { Modal, ModalFooter, Button, ScrollableContainer, SimpleCommitRow } from "@gitbutler/ui";
	import { flip } from "svelte/animate";
	import type { InteractiveIntegrationStep } from "$lib/stacks/stack";

	type Props = {
		modalRef: Modal | undefined;
		projectId: string;
		stackId: string | undefined;
		branchName: string;
	};

	let { modalRef = $bindable(), projectId, stackId, branchName }: Props = $props();

	function closeModal() {
		modalRef?.close();
	}

	const clipboardService = inject(CLIPBOARD_SERVICE);
	const stackService = inject(STACK_SERVICE);
	const [integrate, integrating] = stackService.integrateBranchWithSteps;
	const initialIntegrationSteps = stackService.initialIntegrationSteps(
		projectId,
		stackId,
		branchName,
	);

	let editableSteps = $derived(initialIntegrationSteps.response ?? []);

	const FLIP_ANIMATION_DURATION = 150;
	const MAX_SCROLL_HEIGHT = "50vh";

	function skipStepById(stepId: string, commitId: string) {
		editableSteps = updateStepType(editableSteps, stepId, commitId, "skip");
	}
	function pickStepById(stepId: string, commitId: string) {
		editableSteps = updateStepType(editableSteps, stepId, commitId, "pick");
	}
	function pickUpstreamFromStep(stepId: string, commitId: string, upstreamCommitId: string) {
		editableSteps = pickUpstreamStep(editableSteps, stepId, commitId, upstreamCommitId);
	}
	function pickLocalFromStep(stepId: string, commitId: string) {
		editableSteps = pickLocalStep(editableSteps, stepId, commitId);
	}

	async function getCommitMessage(commitIds: string[]): Promise<string> {
		if (stackId === undefined) return "";
		const commitDetails = await stackService.fetchCommitsByIds(projectId, stackId, commitIds);
		return commitDetails.map((c) => c.message).join("\n\n");
	}
	async function squashStepById(stepId: string, commitIds: string[]) {
		const stepIndex = editableSteps.findIndex((s) => s.subject.id === stepId);
		if (stepIndex === -1 || stepIndex >= editableSteps.length - 1) return;
		const targetStepInfo = getStepCommitInfo(editableSteps[stepIndex + 1]!);
		const combinedCommits = [...commitIds, ...targetStepInfo.commitIds];
		const squashMessage = await getCommitMessage(combinedCommits);
		editableSteps = squashStepInto(editableSteps, stepId, commitIds, squashMessage);
	}
	async function splitOffCommitFromStep(stepId: string, commitId: string) {
		const step = editableSteps.find((s) => s.subject.id === stepId);
		if (!step || step.type !== "squash") return;
		const { commits } = step.subject;
		const commitIndex = commits.indexOf(commitId);
		if (commitIndex <= 0 || !commits.includes(commitId)) return;
		const firstGroup = commits.slice(0, commitIndex);
		const secondGroup = commits.slice(commitIndex);
		const firstGroupMessage = firstGroup.length > 1 ? await getCommitMessage(firstGroup) : "";
		const secondGroupMessage = secondGroup.length > 1 ? await getCommitMessage(secondGroup) : "";
		editableSteps = splitStepAtCommit(
			editableSteps,
			stepId,
			commitId,
			firstGroupMessage,
			secondGroupMessage,
		);
	}
	async function handleIntegrate() {
		if (stackId === undefined) {
			throw new Error("Stack ID is undefined");
		}
		await integrate({
			projectId,
			stackId,
			branchName,
			steps: editableSteps,
		});
		closeModal();
	}
</script>

<Modal bind:this={modalRef} title="Integrate the upstream changes" noPadding width={500}>
	<div class="branch-integration__content">
		<p class="text=13">
			Review and adjust the integration if needed.<br />
			This is what the outcome of the integration will look like.
		</p>
		<div class="branch-integration__steps">
			<ScrollableContainer maxHeight={MAX_SCROLL_HEIGHT}>
				{#each editableSteps as step (step.subject.id)}
					<div
						class="branch-integration__commit-wrap"
						animate:flip={{ duration: FLIP_ANIMATION_DURATION }}
					>
						{@render genericStep(step)}
					</div>
				{/each}
			</ScrollableContainer>
		</div>
	</div>
	<ModalFooter>
		<Button kind="outline" type="reset" onclick={closeModal}>Cancel</Button>
		<Button
			style="pop"
			type="submit"
			onclick={handleIntegrate}
			loading={integrating.current.isLoading}>Integrate changes</Button
		>
	</ModalFooter>
</Modal>

<!-- Snippets from InteractiveBranchIntegration.svelte -->
{#snippet commitItemTemplate(
	commit: Commit | UpstreamCommit,
	stepId: string,
	commitId: string,
	stepType: "pick" | "skip" | "squash" | "pickUpstream",
	isLastInSquash: boolean = false,
	isFirstInSquash: boolean = false,
	squashCommits: string[] = [],
)}
	{@const isSkipStep = stepType === "skip"}
	{@const hideCommitDot = stepType === "squash" && !isFirstInSquash}
	{@const upstreamSha = stepType !== "pickUpstream" ? extractUpstreamCommitId(commit) : undefined}
	{@render commitLine(commit, hideCommitDot, stepType === "pickUpstream")}
	<div class="branch-integration__commit-content">
		<SimpleCommitRow
			author={commit.author.name}
			date={commitCreatedAtDate(commit)}
			title={commit.message}
			sha={commit.id}
			{upstreamSha}
			onCopy={() => {
				clipboardService.write(commit.id, {
					message: "Commit SHA copied",
				});
			}}
			onCopyUpstream={() => {
				clipboardService.write(upstreamSha ?? "", {
					message: "Upstream commit SHA copied",
				});
			}}
			onlyContent
		/>
		{#if stepType === "squash" && !isFirstInSquash}
			<div class="branch-integration__split-off-button">
				<Button
					icon="cut"
					kind="outline"
					size="tag"
					reversedDirection
					tooltip="Split squashed commits at this point"
					onclick={() => splitOffCommitFromStep(stepId, commit.id)}
					disabled={isSkipStep}
				>
					Split off
				</Button>
			</div>
		{/if}
		<div class="branch-integration__commit-actions">
			{#if stepType === "squash" && isLastInSquash}
				<Button
					kind="outline"
					size="tag"
					tooltip="Squash these commits into below"
					icon="commit-arrow-down"
					onclick={() => squashStepById(stepId, squashCommits)}
					disabled={isSkipStep}
				>
					Squash down
				</Button>
				{@render shiftActions(stepId, isSkipStep)}
			{/if}
			{#if stepType === "skip"}
				<Button
					kind="outline"
					size="tag"
					tooltip="Pick this commit"
					icon="eye"
					onclick={() => pickStepById(stepId, commitId)}
					disabled={false}
				>
					Pick
				</Button>
				{@render commitActions(stepId, commitId, true)}
				{@render shiftActions(stepId, true)}
			{:else if stepType === "pick"}
				{#if upstreamSha}
					<Button
						kind="outline"
						size="tag"
						icon="target"
						tooltip="Pick the upstream commit instead"
						onclick={() => pickUpstreamFromStep(stepId, commitId, upstreamSha)}
					>
						Pick upstream
					</Button>
				{/if}
				{@render commitActions(stepId, commitId, false)}
				{@render shiftActions(stepId, false)}
			{:else if stepType === "pickUpstream"}
				<Button
					kind="outline"
					size="tag"
					icon="target"
					tooltip="Pick the local commit instead"
					onclick={() => pickLocalFromStep(stepId, commitId)}
				>
					Pick local
				</Button>
				{@render commitActions(stepId, commit.id, false)}
				{@render shiftActions(stepId, false)}
			{/if}
		</div>
	</div>
{/snippet}

{#snippet genericStep(step: InteractiveIntegrationStep)}
	{@const isSquashStep = step.type === "squash"}
	{@const isIndividualStep = step.type === "pick" || step.type === "skip"}
	{#if isSquashStep}
		{@const commitsQuery = stackService.commitsByIds(projectId, stackId, step.subject.commits)}
		<ReduxResult {projectId} result={commitsQuery.result}>
			{#snippet children(commits)}
				{#each commits as commit, commitIndex (commit.id)}
					{@const isLastCommit = commitIndex === commits.length - 1}
					{@const isFirstCommit = commitIndex === 0}
					<div class="branch-integration__commit">
						{@render commitItemTemplate(
							commit,
							step.subject.id,
							commit.id,
							"squash",
							isLastCommit,
							isFirstCommit,
							step.subject.commits,
						)}
					</div>
					{#if !isLastCommit}
						<div class="branch-integration__commit-divider dotted"></div>
					{/if}
				{/each}
			{/snippet}
		</ReduxResult>
	{:else if isIndividualStep}
		{@const commitDetails = stackService.commitById(projectId, stackId, step.subject.commitId)}
		<ReduxResult {projectId} result={commitDetails.result}>
			{#snippet children(commit)}
				{@const isSkipStep = step.type === "skip"}
				<div class="branch-integration__commit" class:skipped={isSkipStep}>
					{@render commitItemTemplate(
						commit,
						step.subject.id,
						step.subject.commitId,
						step.type,
						false,
						false,
					)}
				</div>
			{/snippet}
		</ReduxResult>
	{:else if step.type === "pickUpstream"}
		{@const commitDetails = stackService.commitDetails(projectId, step.subject.upstreamCommitId)}
		{@const localCommitDetails = stackService.commitById(projectId, stackId, step.subject.commitId)}
		<ReduxResult {projectId} result={commitDetails.result}>
			{#snippet loading()}
				<!-- Show local commit data while loading upstream commit to prevent flickering -->
				<ReduxResult {projectId} result={localCommitDetails.result}>
					{#snippet children(localCommit)}
						<div class="branch-integration__commit">
							{@render commitItemTemplate(
								localCommit,
								step.subject.id,
								step.subject.commitId,
								step.type,
								false,
								false,
							)}
						</div>
					{/snippet}
				</ReduxResult>
			{/snippet}
			{#snippet children(commit)}
				<div class="branch-integration__commit">
					{@render commitItemTemplate(
						commit,
						step.subject.id,
						step.subject.commitId,
						step.type,
						false,
						false,
					)}
				</div>
			{/snippet}
		</ReduxResult>
	{/if}
{/snippet}

{#snippet commitActions(stepId: string, commitId: string, disabled: boolean = false)}
	{#if !disabled}
		<Button
			kind="outline"
			size="tag"
			icon="eye-closed"
			tooltip="Don't pick this commit"
			onclick={() => skipStepById(stepId, commitId)}
			{disabled}
		>
			Skip
		</Button>
	{/if}
	<Button
		kind="outline"
		size="tag"
		icon="commit-arrow-down"
		tooltip="Squash this commit into the one below"
		onclick={() => squashStepById(stepId, [commitId])}
		{disabled}
	>
		Squash down
	</Button>
{/snippet}

{#snippet shiftActions(stepId: string, disabled: boolean = false)}
	<div class="branch-integration__move-buttons">
		<Button
			kind="outline"
			tooltip="Move this commit up"
			class="branch-integration__move-buttons__up"
			size="tag"
			icon="arrow-up"
			disabled={!canShiftStepUp(editableSteps, stepId) || disabled}
			onclick={() => (editableSteps = shiftStepUp(editableSteps, stepId))}
		/>
		<Button
			kind="outline"
			tooltip="Move this commit down"
			class="branch-integration__move-buttons__down"
			size="tag"
			icon="arrow-down"
			disabled={!canShiftStepDown(editableSteps, stepId) || disabled}
			onclick={() => (editableSteps = shiftStepDown(editableSteps, stepId))}
		/>
	</div>
{/snippet}

{#snippet commitLine(
	commit: Commit | UpstreamCommit,
	hideCommitDot: boolean = true,
	overrideIsRemote: boolean = false,
)}
	{#if isCommit(commit) && !overrideIsRemote}
		<CommitTimelineNode
			commitStatus={commit.state.type}
			dotOnTop
			hideDot={hideCommitDot}
			diverged={commit.state.type === "LocalAndRemote" && commit.state.subject !== commit.id}
		/>
	{:else}
		<CommitTimelineNode hideDot={hideCommitDot} dotOnTop commitStatus="Remote" diverged={false} />
	{/if}
{/snippet}

<style lang="postcss">
	.branch-integration__content {
		display: flex;
		flex-direction: column;
		padding: 0 16px 16px 16px;
		gap: 12px;
	}
	.branch-integration__steps {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}
	.branch-integration__commit-wrap {
		border-bottom: 1px solid var(--clr-border-2);
		&:last-child {
			border-bottom: none;
		}
	}
	.branch-integration__commit {
		display: flex;
		z-index: var(--z-ground);
		position: relative;
		&.skipped {
			background-color: var(--clr-bg-2);
		}
	}
	.branch-integration__commit-divider {
		height: 0;
		border-bottom: 1px solid var(--clr-border-2);
		&.dotted {
			height: 1px;
			border-bottom: none;
			background: repeating-linear-gradient(
				to right,
				var(--clr-border-2) 0px,
				var(--clr-border-2) 2px,
				transparent 2px,
				transparent 4px
			);
		}
	}
	.branch-integration__split-off-button {
		z-index: var(--z-lifted);
		position: absolute;
		top: 0;
		right: 20px;
		transform: translateY(-50%);
		border-radius: var(--radius-btn);
		background-color: var(--clr-bg-1);
	}
	.branch-integration__commit-content {
		display: flex;
		flex-direction: column;
		padding: 14px;
		padding-left: 0;
		overflow: hidden;
		gap: 12px;
	}
	.branch-integration__commit-actions {
		display: flex;
		align-items: center;
		gap: 4px;
	}
	.branch-integration__move-buttons {
		display: flex;
	}
	:global(.branch-integration__move-buttons__up) {
		border-top-right-radius: 0;
		border-bottom-right-radius: 0;
	}
	:global(.branch-integration__move-buttons__down) {
		border-left: none !important;
		border-top-left-radius: 0;
		border-bottom-left-radius: 0;
	}
</style>
