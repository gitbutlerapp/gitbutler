<script lang="ts">
	import CommitLine from '$components/CommitLine.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { CLIPBOARD_SERVICE } from '$lib/backend/clipboard';
	import {
		extractUpstreamCommitId,
		isCommit,
		type Commit,
		type UpstreamCommit
	} from '$lib/branches/v3';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';
	import { Modal, ModalFooter, Button, ScrollableContainer, SimpleCommitRow } from '@gitbutler/ui';
	import { flip } from 'svelte/animate';
	import type { InteractiveIntegrationStep } from '$lib/stacks/stack';

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

	// --- Begin InteractiveBranchIntegration logic ---
	const clipboardService = inject(CLIPBOARD_SERVICE);
	const stackService = inject(STACK_SERVICE);
	const [integrate, integrating] = stackService.integrateBranchWithSteps;
	const initialIntegrationSteps = stackService.initialIntegrationSteps(
		projectId,
		stackId,
		branchName
	);

	let editableSteps = $derived(initialIntegrationSteps.current?.data ?? []);

	// Constants
	const FLIP_ANIMATION_DURATION = 150;
	const MAX_SCROLL_HEIGHT = '50vh';

	// Helper functions for step manipulation
	function updateStepType(stepId: string, commitId: string, newType: 'pick' | 'skip') {
		editableSteps = editableSteps.map((step) => {
			if (step.subject.id === stepId) {
				return { type: newType, subject: { id: stepId, commitId } };
			}
			return step;
		});
	}

	function pickUpstreamFromStep(stepId: string, commitId: string, upstreamCommitId: string) {
		editableSteps = editableSteps.map((step) => {
			if (step.subject.id === stepId) {
				return {
					type: 'pickUpstream',
					subject: { id: stepId, commitId, upstreamCommitId }
				};
			}
			return step;
		});
	}

	function pickLocalFromStep(stepId: string, commitId: string) {
		editableSteps = editableSteps.map((step) => {
			if (step.subject.id === stepId) {
				return { type: 'pick', subject: { id: stepId, commitId } };
			}
			return step;
		});
	}

	function skipStepById(stepId: string, commitId: string) {
		updateStepType(stepId, commitId, 'skip');
	}
	function pickStepById(stepId: string, commitId: string) {
		updateStepType(stepId, commitId, 'pick');
	}
	async function getCommitMessage(commitIds: string[]): Promise<string> {
		if (stackId === undefined) return '';
		const commitDetails = await stackService.fetchCommitsByIds(projectId, stackId, commitIds);
		return commitDetails.map((c) => c.message).join('\n\n');
	}
	function getStepCommitInfo(step: InteractiveIntegrationStep): {
		id: string;
		commitIds: string[];
	} {
		const id = step.subject.id;
		switch (step.type) {
			case 'pickUpstream':
				return { id, commitIds: [step.subject.upstreamCommitId] };
			case 'pick':
			case 'skip':
				return { id, commitIds: [step.subject.commitId] };
			case 'squash':
				return { id, commitIds: step.subject.commits };
		}
	}
	async function squashStepById(stepId: string, commitIds: string[]) {
		const stepIndex = editableSteps.findIndex((step) => step.subject.id === stepId);
		const isValidSquashOperation = stepIndex !== -1 && stepIndex < editableSteps.length - 1;
		if (!isValidSquashOperation) {
			return;
		}
		const newSteps = structuredClone(editableSteps);
		const stepToSquash = newSteps[stepIndex];
		const stepToBeSquashedInto = newSteps[stepIndex + 1];
		if (!stepToSquash || !stepToBeSquashedInto) {
			return;
		}
		const targetStepInfo = getStepCommitInfo(stepToBeSquashedInto);
		const combinedCommits = [...commitIds, ...targetStepInfo.commitIds];
		const squashMessage = await getCommitMessage(combinedCommits);
		newSteps.splice(stepIndex, 2, {
			type: 'squash',
			subject: {
				id: targetStepInfo.id,
				commits: combinedCommits,
				message: squashMessage
			}
		});
		editableSteps = newSteps;
	}
	async function splitOffCommitFromStep(stepId: string, commitId: string) {
		const stepIndex = editableSteps.findIndex((step) => step.subject.id === stepId);
		if (stepIndex === -1) return;
		const newSteps = structuredClone(editableSteps);
		const stepToSplit = newSteps[stepIndex];
		if (!stepToSplit || stepToSplit.type !== 'squash') {
			return;
		}
		const { commits } = stepToSplit.subject;
		const canSplitCommits = commits.length > 1 && commits.includes(commitId);
		if (!canSplitCommits) return;
		const commitIndex = commits.indexOf(commitId);
		if (commitIndex === -1) return;
		const firstGroup = commits.slice(0, commitIndex);
		const secondGroup = commits.slice(commitIndex);
		if (firstGroup.length === 0) return;
		if (firstGroup.length === 1) {
			newSteps[stepIndex] = {
				type: 'pick',
				subject: { id: stepId, commitId: firstGroup[0]! }
			};
		} else {
			const firstGroupMessage = await getCommitMessage(firstGroup);
			newSteps[stepIndex] = {
				type: 'squash',
				subject: {
					id: stepId,
					commits: firstGroup,
					message: firstGroupMessage
				}
			};
		}
		if (secondGroup.length === 1) {
			const newPickStep = {
				type: 'pick' as const,
				subject: {
					id: crypto.randomUUID(),
					commitId: secondGroup[0]!
				}
			};
			newSteps.splice(stepIndex + 1, 0, newPickStep);
		} else {
			const secondGroupMessage = await getCommitMessage(secondGroup);
			const newSquashStep = {
				type: 'squash' as const,
				subject: {
					id: crypto.randomUUID(),
					commits: secondGroup,
					message: secondGroupMessage
				}
			};
			newSteps.splice(stepIndex + 1, 0, newSquashStep);
		}
		editableSteps = newSteps;
	}
	function getStepIndex(stepId: string): number {
		return editableSteps.findIndex((step) => step.subject.id === stepId);
	}
	function canShiftStepUp(stepId: string): boolean {
		const index = getStepIndex(stepId);
		return index > 0;
	}
	function canShiftStepDown(stepId: string): boolean {
		const index = getStepIndex(stepId);
		return index !== -1 && index < editableSteps.length - 1;
	}
	function swapSteps(indexA: number, indexB: number) {
		const newSteps = structuredClone(editableSteps);
		const stepA = newSteps[indexA];
		const stepB = newSteps[indexB];
		if (!stepA || !stepB) return;
		newSteps[indexA] = stepB;
		newSteps[indexB] = stepA;
		editableSteps = newSteps;
	}
	function shiftStepDown(stepId: string) {
		const currentIndex = getStepIndex(stepId);
		const isValidShift = currentIndex !== -1 && currentIndex < editableSteps.length - 1;
		if (isValidShift) {
			swapSteps(currentIndex, currentIndex + 1);
		}
	}
	function shiftStepUp(stepId: string) {
		const currentIndex = getStepIndex(stepId);
		const isValidShift = currentIndex > 0;
		if (isValidShift) {
			swapSteps(currentIndex, currentIndex - 1);
		}
	}
	async function handleIntegrate() {
		if (stackId === undefined) {
			throw new Error('Stack ID is undefined');
		}
		await integrate({
			projectId,
			stackId,
			branchName,
			steps: editableSteps
		});
		closeModal();
	}
	// --- End InteractiveBranchIntegration logic ---
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
	stepType: 'pick' | 'skip' | 'squash' | 'pickUpstream',
	isLastInSquash: boolean = false,
	isFirstInSquash: boolean = false,
	squashCommits: string[] = []
)}
	{@const isSkipStep = stepType === 'skip'}
	{@const hideCommitDot = stepType === 'squash' && !isFirstInSquash}
	{@const upstreamSha = stepType !== 'pickUpstream' ? extractUpstreamCommitId(commit) : undefined}
	{@render commitLine(commit, hideCommitDot, stepType === 'pickUpstream')}
	<div class="branch-integration__commit-content">
		<SimpleCommitRow
			author={commit.author.name}
			date={new Date(commit.createdAt)}
			title={commit.message}
			sha={commit.id}
			{upstreamSha}
			onCopy={() => {
				clipboardService.write(commit.id, {
					message: 'Commit SHA copied'
				});
			}}
			onCopyUpstream={() => {
				clipboardService.write(upstreamSha ?? '', {
					message: 'Upstream commit SHA copied'
				});
			}}
			onlyContent
		/>
		{#if stepType === 'squash' && !isFirstInSquash}
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
			{#if stepType === 'squash' && isLastInSquash}
				<Button
					kind="outline"
					size="tag"
					tooltip="Squash these commits into below"
					icon="squash-commit"
					onclick={() => squashStepById(stepId, squashCommits)}
					disabled={isSkipStep}
				>
					Squash down
				</Button>
				{@render shiftActions(stepId, isSkipStep)}
			{/if}
			{#if stepType === 'skip'}
				<Button
					kind="outline"
					size="tag"
					tooltip="Pick this commit"
					icon="eye-shown"
					onclick={() => pickStepById(stepId, commitId)}
					disabled={false}
				>
					Pick
				</Button>
				{@render commitActions(stepId, commitId, true)}
				{@render shiftActions(stepId, true)}
			{:else if stepType === 'pick'}
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
			{:else if stepType === 'pickUpstream'}
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
	{@const isSquashStep = step.type === 'squash'}
	{@const isIndividualStep = step.type === 'pick' || step.type === 'skip'}
	{#if isSquashStep}
		{@const commitsResult = stackService.commitsByIds(projectId, stackId, step.subject.commits)}
		<ReduxResult {projectId} result={commitsResult.current}>
			{#snippet children(commits)}
				{#each commits as commit, commitIndex (commit.id)}
					{@const isLastCommit = commitIndex === commits.length - 1}
					{@const isFirstCommit = commitIndex === 0}
					<div class="branch-integration__commit">
						{@render commitItemTemplate(
							commit,
							step.subject.id,
							commit.id,
							'squash',
							isLastCommit,
							isFirstCommit,
							step.subject.commits
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
		<ReduxResult {projectId} result={commitDetails.current}>
			{#snippet children(commit)}
				{@const isSkipStep = step.type === 'skip'}
				<div class="branch-integration__commit" class:skipped={isSkipStep}>
					{@render commitItemTemplate(
						commit,
						step.subject.id,
						step.subject.commitId,
						step.type,
						false,
						false
					)}
				</div>
			{/snippet}
		</ReduxResult>
	{:else if step.type === 'pickUpstream'}
		{@const commitDetails = stackService.commitDetails(projectId, step.subject.upstreamCommitId)}
		{@const localCommitDetails = stackService.commitById(projectId, stackId, step.subject.commitId)}
		<ReduxResult {projectId} result={commitDetails.current}>
			{#snippet loading()}
				<!-- Show local commit data while loading upstream commit to prevent flickering -->
				<ReduxResult {projectId} result={localCommitDetails.current}>
					{#snippet children(localCommit)}
						<div class="branch-integration__commit">
							{@render commitItemTemplate(
								localCommit,
								step.subject.id,
								step.subject.commitId,
								step.type,
								false,
								false
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
						false
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
			icon="eye-hidden"
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
		icon="squash-commit"
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
			disabled={!canShiftStepUp(stepId) || disabled}
			onclick={() => shiftStepUp(stepId)}
		/>
		<Button
			kind="outline"
			tooltip="Move this commit down"
			class="branch-integration__move-buttons__down"
			size="tag"
			icon="arrow-down"
			disabled={!canShiftStepDown(stepId) || disabled}
			onclick={() => shiftStepDown(stepId)}
		/>
	</div>
{/snippet}

{#snippet commitLine(
	commit: Commit | UpstreamCommit,
	hideCommitDot: boolean = true,
	overrideIsRemote: boolean = false
)}
	{#if isCommit(commit) && !overrideIsRemote}
		<CommitLine
			commitStatus={commit.state.type}
			alignDot="start"
			hideDot={hideCommitDot}
			diverged={commit.state.type === 'LocalAndRemote' && commit.state.subject !== commit.id}
		/>
	{:else}
		<CommitLine hideDot={hideCommitDot} alignDot="start" commitStatus="Remote" diverged={false} />
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
