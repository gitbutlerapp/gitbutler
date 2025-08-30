<script lang="ts">
	import CommitLine from '$components/CommitLine.svelte';
	import CommitTitle from '$components/CommitTitle.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { CLIPBOARD_SERVICE } from '$lib/backend/clipboard';
	import { isCommit, type Commit, type UpstreamCommit } from '$lib/branches/v3';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/shared/context';
	import { Avatar, Button, Icon, ModalFooter, ScrollableContainer, Tooltip } from '@gitbutler/ui';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import { flip } from 'svelte/animate';
	import type { InteractiveIntegrationStep } from '$lib/stacks/stack';

	type Props = {
		projectId: string;
		stackId: string | undefined;
		branchName: string;
		closeModal: () => void;
	};

	const { projectId, stackId, branchName, closeModal }: Props = $props();

	const userService = inject(USER_SERVICE);
	const user = $derived(userService.user);
	const clipboardService = inject(CLIPBOARD_SERVICE);
	const stackService = inject(STACK_SERVICE);
	const [integrate, integrating] = stackService.integrateBranchWithSteps;
	const initialIntegrationSteps = stackService.initialIntegrationSteps(
		projectId,
		stackId,
		branchName
	);

	let editableSteps = $derived(initialIntegrationSteps.current?.data ?? []);

	function getGravatarUrl(email: string, existingGravatarUrl: string): string {
		if ($user?.email === undefined) {
			return existingGravatarUrl;
		}
		if (email === $user.email) {
			return $user.picture ?? existingGravatarUrl;
		}
		return existingGravatarUrl;
	}

	function skipStepById(stepId: string, commitId: string) {
		editableSteps = editableSteps.map((step) => {
			if (step.subject.id === stepId) {
				return { type: 'skip', subject: { id: stepId, commitId } };
			}
			return step;
		});
	}

	function pickStepById(stepId: string, commitId: string) {
		editableSteps = editableSteps.map((step) => {
			if (step.subject.id === stepId) {
				return { type: 'pick', subject: { id: stepId, commitId } };
			}
			return step;
		});
	}

	async function getCommitMessage(commitIds: string[]): Promise<string> {
		if (stackId === undefined) return '';
		const commitDetails = await stackService.fetchCommitsByIds(projectId, stackId, commitIds);
		return commitDetails.map((c) => c.message).join('\n\n');
	}

	function getCommitsAndIdFromStep(step: InteractiveIntegrationStep): [string, string[]] {
		const id = step.subject.id;
		switch (step.type) {
			case 'pick':
			case 'skip':
				return [id, [step.subject.commitId]];
			case 'squash':
				return [id, step.subject.commits];
		}
	}

	async function squashStepById(stepId: string, commitIds: string[]) {
		const stepIndex = editableSteps.findIndex((step) => step.subject.id === stepId);
		if (stepIndex === -1 || stepIndex === stepIndex - 1) {
			// Only squash downwards
			return;
		}

		const newSteps = structuredClone(editableSteps);
		const stepToSquash = newSteps[stepIndex];
		const stepToBeSquashedInto = newSteps[stepIndex + 1];

		if (stepToSquash === undefined) {
			return;
		}

		if (stepToBeSquashedInto === undefined) {
			return;
		}

		const [id, commits] = getCommitsAndIdFromStep(stepToBeSquashedInto);
		const newCommits = [...commitIds, ...commits];
		const message = await getCommitMessage(newCommits);

		newSteps.splice(stepIndex, 2, {
			type: 'squash',
			subject: {
				id,
				commits: newCommits,
				message
			}
		});

		editableSteps = newSteps;
	}

	async function splitOffCommitFromStep(stepId: string, commitId: string) {
		const stepIndex = editableSteps.findIndex((step) => step.subject.id === stepId);
		if (stepIndex === -1) {
			return;
		}

		const newSteps = structuredClone(editableSteps);
		const stepToSplit = newSteps[stepIndex];

		if (stepToSplit === undefined) {
			return;
		}

		if (stepToSplit.type !== 'squash') {
			// Can only split off from a squash step
			return;
		}

		const commits = stepToSplit.subject.commits;

		if (commits.length <= 1) {
			// Can't split off if there's only one commit
			return;
		}

		const commitIndex = commits.indexOf(commitId);
		if (commitIndex === -1) {
			return;
		}

		const newSquashCommits = commits.filter((c) => c !== commitId);
		const message = await getCommitMessage(newSquashCommits);

		newSteps.splice(stepIndex, 1, {
			type: 'squash',
			subject: {
				id: stepId,
				commits: newSquashCommits,
				message: message
			}
		});

		newSteps.splice(stepIndex + 1, 0, {
			type: 'pick',
			subject: {
				// This is not ideal, since all steps are ided in the backend. It's fine for now though
				id: crypto.randomUUID(),
				commitId
			}
		});

		editableSteps = newSteps;
	}

	function canShiftStepUp(stepId: string): boolean {
		const index = editableSteps.findIndex((step) => step.subject.id === stepId);
		return index > 0;
	}

	function canShiftStepDown(stepId: string): boolean {
		const index = editableSteps.findIndex((step) => step.subject.id === stepId);
		return index !== -1 && index < editableSteps.length - 1;
	}

	function shiftStepDown(stepId: string) {
		const index = editableSteps.findIndex((step) => step.subject.id === stepId);
		if (index === -1 || index === editableSteps.length - 1) {
			return;
		}
		const newSteps = structuredClone(editableSteps);
		const temp = newSteps[index];
		const other = newSteps[index + 1];

		if (temp === undefined) {
			return;
		}
		if (other === undefined) {
			return;
		}

		newSteps[index] = other;
		newSteps[index + 1] = temp;
		editableSteps = newSteps;
	}

	function shiftStepUp(stepId: string) {
		const index = editableSteps.findIndex((step) => step.subject.id === stepId);
		if (index <= 0) {
			return;
		}
		const newSteps = structuredClone(editableSteps);
		const temp = newSteps[index];
		const other = newSteps[index - 1];

		if (temp === undefined) {
			return;
		}
		if (other === undefined) {
			return;
		}

		newSteps[index] = other;
		newSteps[index - 1] = temp;
		editableSteps = newSteps;
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
</script>

<div class="branch-integration__content">
	<p class="text=13">
		This is what the outcome of the integration will look like. Please review and adjust the
		integration if needed.
	</p>

	<ScrollableContainer maxHeight="70vh">
		<div class="branch-integration__steps">
			<ReduxResult {projectId} result={initialIntegrationSteps.current}>
				{#snippet children(_)}
					<!-- Use ReduxResult in order to gracefully load and handle the results of the API call. -->
					<!-- But use the editable steps in order to show which are the steps that will be sent to the rust-end -->
					{#each editableSteps as step (step.subject.id)}
						<div animate:flip={{ duration: 150 }}>
							{#if step.type === 'pick'}
								{@render pickStep(step.subject.id, step.subject.commitId)}
							{:else if step.type === 'skip'}
								{@render skipStep(step.subject.id, step.subject.commitId)}
							{:else if step.type === 'squash'}
								{@render squashStepDown(step.subject.id, step.subject.commits)}
							{/if}
						</div>
					{/each}
				{/snippet}
			</ReduxResult>
		</div>
	</ScrollableContainer>
</div>

<ModalFooter>
	<Button kind="outline" type="reset" onclick={closeModal}>Cancel</Button>
	<Button
		style="pop"
		type="submit"
		onclick={handleIntegrate}
		loading={integrating.current.isLoading}>Integrate</Button
	>
</ModalFooter>

{#snippet pickStep(id: string, commitId: string)}
	{@const commitDetails = stackService.commitById(projectId, stackId, commitId)}
	<ReduxResult {projectId} result={commitDetails.current}>
		{#snippet children(commit)}
			<div class="branch-integration__step">
				{@render commitLine(commit)}
				<div class="commit-info">
					<CommitTitle commitMessage={commit.message} truncate className="text-13 text-semibold" />
					{@render commitMetadata(commit)}
				</div>
				<div class="branch-integration__step-actions">
					<Button
						kind="ghost"
						tooltip="Squash this commit into the one below"
						onclick={() => squashStepById(id, [commitId])}>squash down</Button
					>
					<Button
						kind="ghost"
						tooltip="Don't pick this commit"
						onclick={() => skipStepById(id, commitId)}>skip</Button
					>
					{@render shiftActions(id)}
				</div>
			</div>
		{/snippet}
	</ReduxResult>
{/snippet}

{#snippet skipStep(id: string, commitId: string)}
	{@const commitDetails = stackService.commitById(projectId, stackId, commitId)}
	<ReduxResult {projectId} result={commitDetails.current}>
		{#snippet children(commit)}
			<div class="branch-integration__step skipped">
				{@render commitLine(commit)}
				<div class="commit-info">
					<CommitTitle commitMessage={commit.message} truncate className="text-13" />
					{@render commitMetadata(commit)}
				</div>
				<div class="branch-integration__step-actions">
					<Button kind="ghost" tooltip="Pick this commit" onclick={() => pickStepById(id, commitId)}
						>pick</Button
					>
				</div>
			</div>
		{/snippet}
	</ReduxResult>
{/snippet}

{#snippet squashStepDown(id: string, commitIds: string[])}
	{@const commits = stackService.commitsByIds(projectId, stackId, commitIds)}
	<ReduxResult {projectId} result={commits.current}>
		{#snippet children(commits)}
			<div class="branch-integration__step">
				<div class="branch-integration__squash">
					{#each commits as commit (commit.id)}
						<div class="branch-integration__squash-item">
							{@render commitLine(commit)}
							<div class="commit-info">
								<CommitTitle
									commitMessage={commit.message}
									truncate
									className="text-13 text-semibold"
								/>
								{@render commitMetadata(commit)}
							</div>
							<dir>
								<Button
									kind="ghost"
									tooltip="Split off this commit"
									onclick={() => splitOffCommitFromStep(id, commit.id)}>split off</Button
								>
							</dir>
						</div>
					{/each}
				</div>
				<div class="branch-integration__step-actions">
					<Button
						kind="ghost"
						tooltip="Squash these commits into below"
						onclick={() => squashStepById(id, commitIds)}>squash down</Button
					>
					{@render shiftActions(id)}
				</div>
			</div>
		{/snippet}
	</ReduxResult>
{/snippet}

{#snippet shiftActions(id: string)}
	<div class="branch-integration__step-shift">
		<Button
			kind="ghost"
			tooltip="Move this commit up"
			size="tag"
			icon="chevron-up"
			disabled={!canShiftStepUp(id)}
			onclick={() => shiftStepUp(id)}
		/>
		<Button
			kind="ghost"
			tooltip="Move this commit down"
			size="tag"
			icon="chevron-down"
			disabled={!canShiftStepDown(id)}
			onclick={() => shiftStepDown(id)}
		/>
	</div>
{/snippet}

{#snippet commitLine(commit: Commit | UpstreamCommit)}
	{#if isCommit(commit)}
		<!-- Local and Remote commmit -->
		<CommitLine
			slim
			commitStatus={commit.state.type}
			diverged={commit.state.type === 'LocalAndRemote' && commit.state.subject !== commit.id}
		/>
	{:else}
		<!-- Upstream Only commit -->
		<CommitLine slim commitStatus="Remote" diverged={false} />
	{/if}
{/snippet}

{#snippet commitMetadata(commit: Commit | UpstreamCommit)}
	<div class="commit-metadata text-12">
		<Avatar
			size="medium"
			tooltip={commit.author.name}
			srcUrl={getGravatarUrl(commit.author.email, commit.author.gravatarUrl)}
		/>
		<span class="divider">•</span>
		<span>{getTimeAgo(new Date(commit.createdAt))}</span>
		<span class="divider">•</span>
		<Tooltip text="Copy commit SHA">
			<button
				type="button"
				class="copy-sha underline-dotted"
				onclick={() => {
					clipboardService.write(commit.id, {
						message: 'Commit SHA copied'
					});
				}}
			>
				<span>
					{commit.id.substring(0, 7)}
				</span>
				<Icon name="copy-small" />
			</button>
		</Tooltip>
	</div>
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
		gap: 4px;
	}

	.branch-integration__step {
		display: flex;
		padding: 8px;
		padding-left: 4px;
		gap: 4px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);

		&.skipped {
			background-color: var(--clr-bg-2);
		}
	}

	.branch-integration__squash {
		display: flex;
		flex-grow: 1;
		flex-shrink: 0;
		flex-direction: column;
		gap: 8px;
	}

	.branch-integration__squash-item {
		display: flex;
		gap: 4px;
	}

	.branch-integration__step-actions {
		display: flex;
		flex-grow: 1;
		flex-shrink: 0;
		align-items: center;
		justify-content: flex-end;
	}

	.branch-integration__step-shift {
		display: flex;
		flex-direction: column;
	}

	.commit-info {
		display: flex;
		flex-grow: 1;
		flex-shrink: 0;
		flex-direction: column;
		gap: 4px;
	}

	.commit-metadata {
		display: flex;
		align-items: center;
		gap: 4px;
		color: var(--clr-text-2);

		& .divider {
			font-size: 12px;
			opacity: 0.4;
		}
	}

	.copy-sha {
		display: flex;
		align-items: center;
		gap: 2px;
		text-decoration: underline dotted;
	}
</style>
