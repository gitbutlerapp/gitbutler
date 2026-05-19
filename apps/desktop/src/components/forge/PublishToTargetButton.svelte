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
	const upstreamCommits = $derived(baseBranch?.upstreamCommits ?? []);
	const hasUpstreamCommits = $derived(upstreamCommits.length > 0);
	const branchCount = $derived(branches.length);
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
			chipToasts.success(`Published stack to ${remoteName}/${targetBranch}`);
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
			title={`Publish stack to ${targetBranch}`}
			width={520}
			type={hasUpstreamCommits ? "warning" : "info"}
			bind:this={modal}
		>
			<div class="publish-modal">
				{#if hasUpstreamCommits}
					<div class="danger-warning">
						<div class="danger-warning__title">This will overwrite upstream commits</div>
						<p>
							{remoteName}/{targetBranch} has {upstreamCommits.length === 1
								? "1 commit"
								: `${upstreamCommits.length} commits`} that are not in your local target. Publishing
							this stack will force-push the stack head to {targetBranch}.
						</p>
					</div>
				{/if}

				<p class="description">
					GitButler will collapse {branchCount === 1 ? "1 branch" : `${branchCount} branches`}
					into {remoteName}/{targetBranch}, preserving the stack's commit history and order.
				</p>

				{#if hasUpstreamCommits}
					<div class="scroll-wrap">
						<ScrollableContainer maxHeight="13rem">
							{#each upstreamCommits as commit}
								{@const commitUrl = forge.current.commitUrl(commit.id)}
								<SimpleCommitRow
									title={splitMessage(commit.description).title ?? ""}
									sha={commit.id}
									date={new Date(Number(commit.createdAt))}
									author={commit.author.name}
									url={commitUrl}
									onOpen={(url) => urlService.openExternalUrl(url)}
									onCopy={() => clipboardService.write(commit.id, { message: "Commit hash copied" })}
								/>
							{/each}
						</ScrollableContainer>
					</div>
				{/if}
			</div>

			{#snippet controls(close)}
				<div class="controls">
					<Button kind="outline" onclick={close}>Cancel</Button>
					{#if hasUpstreamCommits && !confirmedUpstreamWarning}
						<Button style="warning" onclick={() => (confirmedUpstreamWarning = true)}>
							I understand
						</Button>
					{:else}
						<HoldConfirmButton
							testId={TestId.StackPublishToTargetHoldButton}
							wide
							style={hasUpstreamCommits ? "danger" : "pop"}
							icon="push-all"
							{loading}
							onconfirm={() => publish(close)}
						>
							Hold to publish {commitCount === 1 ? "1 commit" : `${commitCount} commits`}
						</HoldConfirmButton>
					{/if}
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

	.danger-warning {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 12px;
		border: 1px solid var(--fill-danger-bg);
		border-radius: var(--radius-m);
		background: color-mix(in srgb, var(--fill-danger-bg) 12%, transparent);
		color: var(--text-1);
	}

	.danger-warning__title {
		color: var(--fill-danger-bg);
		font-weight: 700;
		font-size: 15px;
	}

	.danger-warning p {
		margin: 0;
		line-height: 1.4;
	}

	.scroll-wrap {
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
	}

	.controls {
		display: flex;
		width: 100%;
		gap: 6px;
	}
</style>
