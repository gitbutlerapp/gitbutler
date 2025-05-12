<script lang="ts">
	import CommitAction from '$components/CommitAction.svelte';
	import CommitsAccordion from '$components/CommitsAccordion.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { StackService, type SeriesIntegrationStrategy } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import type { Commit, UpstreamCommit } from '$lib/branches/v3';
	import type { CommitKey } from '$lib/commits/commit';
	import type { Snippet } from 'svelte';

	const integrationStrategies = {
		default: {
			label: 'Integrate upstream',
			style: 'warning',
			kind: 'solid',
			icon: undefined,
			action: () => integrate()
		},
		reset: {
			label: 'Reset to remote…',
			style: 'ghost',
			kind: 'outline',
			icon: 'warning-small',
			action: confirmReset
		}
	} as const;

	type IntegrationStrategy = keyof typeof integrationStrategies;

	interface Props {
		projectId: string;
		stackId: string;
		branchName: string;
		selectedCommitId?: string;
		empty?: Snippet;
		beforeLocalAndRemote?: Snippet;
		upstreamTemplate?: Snippet<
			[
				{
					commit: UpstreamCommit;
					commitKey: CommitKey;
					first: boolean;
					lastCommit: boolean;
					selected: boolean;
				}
			]
		>;
		localAndRemoteTemplate?: Snippet<
			[
				{
					commit: Commit;
					commitKey: CommitKey;
					first: boolean;
					last: boolean;
					lastCommit: boolean;
					selectedCommitId: string | undefined;
				}
			]
		>;
	}

	let {
		projectId,
		stackId,
		branchName,
		selectedCommitId,
		empty,
		beforeLocalAndRemote,
		localAndRemoteTemplate,
		upstreamTemplate
	}: Props = $props();

	const [stackService] = inject(StackService);
	const [integrateUpstreamCommits, upstreamIntegration] = stackService.integrateUpstreamCommits;

	const localAndRemoteCommits = $derived(stackService.commits(projectId, stackId, branchName));
	const upstreamOnlyCommits = $derived(
		stackService.upstreamCommits(projectId, stackId, branchName)
	);

	let confirmResetModal = $state<ReturnType<typeof Modal>>();

	async function integrate(strategy?: SeriesIntegrationStrategy): Promise<void> {
		await integrateUpstreamCommits({
			projectId,
			stackId,
			seriesName: branchName,
			strategy
		});
	}

	function confirmReset() {
		confirmResetModal?.show();
	}
</script>

<Modal
	bind:this={confirmResetModal}
	title="Reset to remote"
	type="warning"
	width="small"
	onSubmit={async (close) => {
		await integrate('hardreset');
		close();
	}}
>
	<p class="text-13 text-body helper-text">
		This will reset the branch to the state of the remote branch. All local changes will be
		overwritten.
	</p>
	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<Button style="error" type="submit">Reset</Button>
	{/snippet}
</Modal>

{#snippet integrateUpstreamButton(strategy: IntegrationStrategy)}
	{@const { label, icon, style, kind, action } = integrationStrategies[strategy]}
	<Button
		testId={TestId.UpstreamCommitsIntegrateButton}
		{style}
		{kind}
		grow
		{icon}
		reversedDirection
		loading={upstreamIntegration.current.isLoading}
		onclick={action}
	>
		{label}
	</Button>
{/snippet}

<ReduxResult
	{stackId}
	{projectId}
	result={combineResults(upstreamOnlyCommits.current, localAndRemoteCommits.current)}
>
	{#snippet children([upstreamOnlyCommits, localAndRemoteCommits], { stackId })}
		{@const hasRemoteCommits = upstreamOnlyCommits.length > 0}
		{@const hasCommits = localAndRemoteCommits.length > 0}
		{#if !hasCommits}
			{@render empty?.()}
		{/if}
		<div class="commit-list">
			{#if hasRemoteCommits}
				<CommitsAccordion
					testId={TestId.UpstreamCommitsAccordion}
					count={Math.min(upstreamOnlyCommits.length, 3)}
					isLast={!hasCommits}
					type="upstream"
					displayHeader={upstreamOnlyCommits.length > 1}
				>
					{#snippet title()}
						<span class="text-13 text-body text-semibold">Upstream commits</span>
					{/snippet}

					{#if upstreamTemplate}
						{#each upstreamOnlyCommits as commit, i (commit.id)}
							{@const first = i === 0}
							{@const lastCommit = i === upstreamOnlyCommits.length - 1}
							{@const commitKey = { stackId, branchName, commitId: commit.id, upstream: true }}
							{@const selected = selectedCommitId === commit.id}
							{@render upstreamTemplate({ commit, commitKey, first, lastCommit, selected })}
						{/each}
					{/if}

					<CommitAction type="Remote" isLast={!hasCommits}>
						{#snippet action()}
							<!-- TODO: Ability to select other actions would be nice -->
							{@render integrateUpstreamButton('default')}
						{/snippet}
					</CommitAction>
				</CommitsAccordion>
			{/if}

			{#if localAndRemoteTemplate}
				{@render beforeLocalAndRemote?.()}
				{#each localAndRemoteCommits as commit, i (commit.id)}
					{@const first = i === 0}
					{@const last = i === localAndRemoteCommits.length - 1}
					{@const commitKey = { stackId, branchName, commitId: commit.id, upstream: false }}
					{@render localAndRemoteTemplate({
						commit,
						commitKey,
						first,
						last,
						lastCommit: last,
						selectedCommitId
					})}
				{/each}
			{/if}
		</div>
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.commit-list {
		position: relative;
		display: flex;
		flex-direction: column;
		border-radius: 0 0 var(--radius-ml) var(--radius-ml);
	}
</style>
