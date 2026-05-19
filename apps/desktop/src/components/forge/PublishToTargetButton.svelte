<script lang="ts">
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
	import { URL_SERVICE } from "$lib/backend/url";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { splitMessage } from "$lib/commits/commitMessage";
	import { projectRunCommitHooks } from "$lib/config/config";
	import { DEFAULT_FORGE_FACTORY } from "$lib/forge/forgeFactory.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import {
		Button,
		HoldConfirmButton,
		Modal,
		ScrollableContainer,
		SimpleCommitRow,
		TestId,
		chipToasts,
	} from "@gitbutler/ui";
	import type { BranchDetails } from "@gitbutler/but-sdk";

	type Props = {
		projectId: string;
		stackId?: string;
		branches: BranchDetails[];
		disabled?: boolean;
	};

	const { projectId, stackId, branches, disabled = false }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const urlService = inject(URL_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);
	const uiState = inject(UI_STATE);

	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));
	const baseBranch = $derived(baseBranchQuery.response);
	const runHooks = $derived(projectRunCommitHooks(projectId));
	const [pushStackToTarget, pushStackToTargetQuery] = stackService.pushStackToTarget;

	let modal = $state<ReturnType<typeof Modal>>();
	let confirmedUpstreamWarning = $state(false);

	const targetBranch = $derived(baseBranch?.shortName ?? "target");
	const remoteName = $derived(baseBranch?.pushRemoteName ?? baseBranch?.remoteName);
	const targetRef = $derived(remoteName ? `${remoteName}/${targetBranch}` : targetBranch);
	const upstreamCommits = $derived(baseBranch?.upstreamCommits ?? []);
	const hasUpstreamCommits = $derived(upstreamCommits.length > 0);
	const branchCount = $derived(branches.length);
	const stackCommits = $derived(branches.flatMap((branch) => branch.commits));
	const commitCount = $derived(
		branches.reduce((count, branch) => count + branch.commits.length, 0),
	);
	const loading = $derived(pushStackToTargetQuery.current.isLoading);

	function showModal(e: MouseEvent) {
		e.stopPropagation();
		confirmedUpstreamWarning = false;
		modal?.show();
	}

	async function publish(close: () => void) {
		if (!stackId || !baseBranch) return;

		try {
			const result = await pushStackToTarget({
				projectId,
				stackId,
				withForce: hasUpstreamCommits,
				skipForcePushProtection: hasUpstreamCommits,
				runHooks: $runHooks,
			});

			if (result.branchToRemote.length > 0 && remoteName) {
				uiState.project(projectId).branchesToPoll.add(targetBranch);
			}

			close();
			chipToasts.success(`Published stack to ${targetRef}`);
		} catch (error: any) {
			if (error?.code === "GitForcePushProtection") {
				confirmedUpstreamWarning = false;
				return;
			}
			throw error;
		}
	}
</script>

<ReduxResult {projectId} result={baseBranchQuery.result}>
	{#snippet children()}
		<Button
			testId={TestId.StackPublishToTargetButton}
			size="tag"
			kind="outline"
			style={hasUpstreamCommits ? "warning" : "gray"}
			icon="push-all"
			tooltip={`Collapse this stack and publish it to ${remoteName}/${targetBranch}`}
			onclick={showModal}
			disabled={disabled || !stackId || !baseBranch}
		>
			Publish to {targetBranch}
		</Button>

		<Modal
			title={hasUpstreamCommits
				? `Publish stack on top of ${targetRef}?`
				: `Publish stack to ${targetRef}`}
			width={560}
			type={hasUpstreamCommits ? "warning" : "info"}
			bind:this={modal}
		>
			<div class="publish-modal">
				{#if hasUpstreamCommits}
					<div class="publish-warning">
						<div class="publish-warning__title">
							Your stack will be placed after upstream commits
						</div>
						<p>
							{targetRef} has {upstreamCommits.length === 1
								? "1 commit"
								: `${upstreamCommits.length} commits`} that are not in your local target. GitButler will
							preserve those commits and publish your stack on top of them.
						</p>
						<p>
							This can still override file contents. If your stack changes the same paths as the
							upstream commits, your stack's version becomes the final version. Binary files cannot
							be merged safely, so your version may replace the upstream version.
						</p>
					</div>
				{/if}

				<div class="summary">
					<div class="summary__row">
						<span>Target branch</span>
						<strong>{targetRef}</strong>
					</div>
					{#if hasUpstreamCommits}
						<div class="summary__row">
							<span>Upstream commits preserved</span>
							<strong>{upstreamCommits.length}</strong>
						</div>
					{/if}
					<div class="summary__row">
						<span>Stack commits published on top</span>
						<strong>{commitCount}</strong>
					</div>
					<div class="summary__row">
						<span>Branches collapsed</span>
						<strong>{branchCount}</strong>
					</div>
				</div>

				<div class="flow" aria-label="Publish order">
					{#if hasUpstreamCommits}
						<span>{targetRef} now</span>
						<span class="flow__arrow">-></span>
						<span>{upstreamCommits.length} upstream commits</span>
						<span class="flow__arrow">-></span>
					{/if}
					<strong>Your stack commits</strong>
				</div>

				<p class="description">
					GitButler will collapse {branchCount === 1 ? "1 branch" : `${branchCount} branches`}
					into {targetRef}, preserving the stack's commit history and order.
				</p>

				{#if hasUpstreamCommits}
					<div class="commit-section">
						<div class="commit-section__header">
							<span>Upstream commits that will be preserved</span>
							<strong>{upstreamCommits.length}</strong>
						</div>
						<div class="scroll-wrap">
							<ScrollableContainer maxHeight="9.5rem">
								{#each upstreamCommits as commit}
									{@const commitUrl = forge.current.commitUrl(commit.id)}
									<SimpleCommitRow
										title={splitMessage(commit.description).title ?? ""}
										sha={commit.id}
										date={new Date(Number(commit.createdAt))}
										author={commit.author.name}
										url={commitUrl}
										onOpen={(url) => urlService.openExternalUrl(url)}
										onCopy={() =>
											clipboardService.write(commit.id, { message: "Commit hash copied" })}
									/>
								{/each}
							</ScrollableContainer>
						</div>
					</div>
				{/if}

				{#if stackCommits.length > 0}
					<div class="commit-section">
						<div class="commit-section__header">
							<span>Stack commits that will be published on top</span>
							<strong>{stackCommits.length}</strong>
						</div>
						<div class="scroll-wrap">
							<ScrollableContainer maxHeight={hasUpstreamCommits ? "9.5rem" : "13rem"}>
								{#each stackCommits as commit}
									{@const commitUrl = forge.current.commitUrl(commit.id)}
									<SimpleCommitRow
										title={splitMessage(commit.message).title ?? ""}
										sha={commit.id}
										date={new Date(Number(commit.createdAt))}
										author={commit.author.name}
										url={commitUrl}
										onOpen={(url) => urlService.openExternalUrl(url)}
										onCopy={() =>
											clipboardService.write(commit.id, { message: "Commit hash copied" })}
									/>
								{/each}
							</ScrollableContainer>
						</div>
					</div>
				{:else if hasUpstreamCommits}
					<div class="scroll-wrap">
						<p class="empty-message">This stack has no commits to publish.</p>
					</div>
				{/if}
			</div>

			{#snippet controls(close)}
				<div class="controls">
					<Button kind="outline" onclick={close}>Cancel</Button>
					<div
						class:expanded-action={!hasUpstreamCommits || confirmedUpstreamWarning}
						class="publish-action"
					>
						{#if hasUpstreamCommits && !confirmedUpstreamWarning}
							<div class="publish-action__content">
								<Button wide style="warning" onclick={() => (confirmedUpstreamWarning = true)}>
									Continue to publish
								</Button>
							</div>
						{:else}
							<div class="publish-action__content">
								<HoldConfirmButton
									testId={TestId.StackPublishToTargetHoldButton}
									wide
									style={hasUpstreamCommits ? "danger" : "pop"}
									icon="push-all"
									{loading}
									onconfirm={() => publish(close)}
								>
									Hold to publish {hasUpstreamCommits
										? "on top"
										: commitCount === 1
											? "1 commit"
											: `${commitCount} commits`}
								</HoldConfirmButton>
							</div>
						{/if}
					</div>
				</div>
			{/snippet}
		</Modal>
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.publish-modal {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	.description {
		margin: 0;
		color: var(--text-2);
	}

	.publish-warning {
		display: flex;
		flex-direction: column;
		padding: 12px;
		gap: 6px;
		border: 1px solid var(--fill-warn-bg);
		border-radius: var(--radius-m);
		background: color-mix(in srgb, var(--fill-warn-bg) 12%, transparent);
		color: var(--text-1);
	}

	.publish-warning__title {
		color: var(--fill-warn-bg);
		font-weight: 700;
		font-size: 15px;
	}

	.publish-warning p {
		margin: 0;
		line-height: 1.4;
	}

	.summary {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
	}

	.summary__row {
		display: flex;
		justify-content: space-between;
		padding: 8px 12px;
		gap: 12px;
		border-bottom: 1px solid var(--border-2);
		color: var(--text-2);
	}

	.summary__row:last-child {
		border-bottom: none;
	}

	.summary__row strong {
		color: var(--text-1);
		font-weight: 600;
		text-align: right;
	}

	.flow {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		padding: 10px 12px;
		gap: 6px;
		border: 1px dashed var(--border-2);
		border-radius: var(--radius-m);
		color: var(--text-2);
	}

	.flow strong {
		color: var(--text-1);
		font-weight: 600;
	}

	.flow__arrow {
		color: var(--text-3);
	}

	.commit-section {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.commit-section__header {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		color: var(--text-2);
		font-weight: 600;
	}

	.commit-section__header strong {
		color: var(--text-1);
	}

	.scroll-wrap {
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
	}

	.empty-message {
		margin: 0;
		padding: 12px;
		color: var(--text-2);
	}

	.controls {
		display: flex;
		width: 100%;
		gap: 6px;
	}

	.publish-action {
		--publish-action-resize-transition: 120ms var(--motion-ease-standard);

		display: grid;
		flex: 0 1 9rem;
		min-width: 9rem;
		transition:
			flex-basis var(--publish-action-resize-transition),
			flex-grow var(--publish-action-resize-transition),
			min-width var(--publish-action-resize-transition);
	}

	.publish-action.expanded-action {
		flex: 1 1 0;
		min-width: 0;
	}

	.publish-action__content {
		display: flex;
		min-width: 0;
	}
</style>
