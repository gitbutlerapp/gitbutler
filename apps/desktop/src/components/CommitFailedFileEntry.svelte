<script lang="ts">
	import { goto } from "$app/navigation";
	import ReduxResult from "$components/ReduxResult.svelte";
	import { showError, showToast } from "$lib/notifications/toasts";
	import { branchesPath } from "$lib/routes/routes.svelte";
	import { handleCreateBranchFromBranchOutcome } from "$lib/stacks/stack";
	import { DEPENDENCY_SERVICE } from "$lib/dependencies/dependencyService.svelte";
	import { SETTINGS } from "$lib/settings/userSettings";
	import { type RejectionReason } from "$lib/stacks/stackService.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { AsyncButton, Button, FileName, HunkDiff, Icon, Tooltip } from "@gitbutler/ui";

	type Props = {
		path: string;
		reason: RejectionReason;
		projectId: string;
	};

	const { path, reason, projectId }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const userSettings = inject(SETTINGS);
	const dependencyService = inject(DEPENDENCY_SERVICE);

	let isFolded = $state(true);
	let applyingStackId = $state<string | null>(null);
	let locatingCommitId = $state<string | null>(null);
	let ignoredLocks = $state<Record<string, true>>({});

	function reasonRelatedToDependencyInfo(reason: RejectionReason): boolean {
		return (
			reason === "cherryPickMergeConflict" ||
			reason === "workspaceMergeConflict" ||
			reason === "workspaceMergeConflictOfUnrelatedFile"
		);
	}

	function lockKey(lock: {
		commitId: string;
		target: { type: string; subject?: string };
	}): string {
		return `${lock.commitId}:${lock.target.type}:${lock.target.subject ?? ""}`;
	}

	function ignoreLock(lock: {
		commitId: string;
		target: { type: string; subject?: string };
	}) {
		ignoredLocks = {
			...ignoredLocks,
			[lockKey(lock)]: true,
		};
	}

	function isIgnoredLock(lock: {
		commitId: string;
		target: { type: string; subject?: string };
	}): boolean {
		return ignoredLocks[lockKey(lock)] === true;
	}

	async function applyKnownButUnappliedStack(stackId: string, branchName: string) {
		applyingStackId = stackId;
		try {
			const outcome = await stackService.createVirtualBranchFromBranch({
				projectId,
				branch: `refs/heads/${branchName}`,
			});
			handleCreateBranchFromBranchOutcome(outcome);
			showToast({
				title: "Stack applied to workspace",
				message: `Applied stack '${branchName}'. Retry the commit after conflicts are resolved.`,
				style: "info",
			});
		} catch (error) {
			showError("Failed to apply stack", error);
		} finally {
			applyingStackId = null;
		}
	}

	async function recoverStackByCommit(commitId: string) {
		locatingCommitId = commitId;
		try {
			const allStacks = await stackService.fetchAllStacks(projectId);
			if (!allStacks || allStacks.length === 0) {
				showToast({
					title: "No stacks available to recover",
					message: "No stacks were found. Open Branches to create/apply a stack and retry.",
					style: "warning",
				});
				return;
			}

			for (const stack of allStacks) {
				if (!stack.id) continue;
				const branches = await stackService.fetchBranches(projectId, stack.id);
				const commitFound = branches?.some((branch) =>
					branch.commits.some((commit) => commit.id === commitId),
				);
				if (!commitFound) continue;

				const stackHead = stack.heads.at(0)?.name;
				if (!stackHead) {
					showToast({
						title: "Stack found but no branch head",
						message: `Commit ${commitId.substring(0, 7)} belongs to a stack without a visible head branch.
Open Branches to inspect and recover manually.`,
						style: "warning",
					});
					return;
				}

				await applyKnownButUnappliedStack(stack.id, stackHead);
				return;
			}

			showToast({
				title: "Commit could not be mapped to a stack",
				message: `Commit ${commitId.substring(0, 7)} was not found in known stacks.
It may be orphaned history. Open Branches to create a recovery stack/branch.`,
				style: "warning",
			});
		} catch (error) {
			showError("Failed to recover unknown stack", error);
		} finally {
			locatingCommitId = null;
		}
	}
</script>

{#if reasonRelatedToDependencyInfo(reason)}
	<!-- In some cases, the dependency information is relevant to the cause of commit rejection.
	 Show the relevant diff locks in that case. -->
	{@const fileDependencies = dependencyService.fileDependencies(projectId, path)}
	<div class="commit-failed__file-entry">
		<button
			type="button"
			class="commit-failed__file-entry__header clickable"
			onclick={() => (isFolded = !isFolded)}
		>
			<FileName filePath={path} textSize="13" />

			<div class="commit-failed__file-entry__header__unfold-action">
				<span class="text-12 text-semibold"
					>{isFolded ? "Show" : "Hide"}
					hunks ({#if fileDependencies.response}
						{fileDependencies.response.dependencies.length}
					{:else}
						0
					{/if})</span
				>

				<Icon name={isFolded ? "chevron-down" : "chevron-up"} />
			</div>
		</button>

		{#if !isFolded}
			<div class="commit-failed__file-entry-dependencies">
				<ReduxResult {projectId} result={fileDependencies.result}>
					{#snippet children(fileDependencies)}
						{#each fileDependencies.dependencies as dependency, i}
							<HunkDiff
								filePath="test.tsx"
								tabSize={$userSettings.tabSize}
								wrapText={$userSettings.wrapText}
								diffFont={$userSettings.diffFont}
								diffLigatures={$userSettings.diffLigatures}
								strongContrast={$userSettings.strongContrast}
								colorBlindFriendly={$userSettings.colorBlindFriendly}
								inlineUnifiedDiffs={$userSettings.inlineUnifiedDiffs}
								hunkStr={dependency.hunk.diff}
								draggingDisabled
							/>

							<div class="text-12 commit-failed__file-entry__dependency-locks">
								<div class="commit-failed__file-entry__dependency-locks__label">
									<Icon name="locked" />
									<span class="clr-text-2">Depends on:</span>
								</div>
								<div class="commit-failed__file-entry__dependency-locks__content">
									{#each dependency.locks as lock}
										{#if !isIgnoredLock(lock)}
											{#if lock.target.type === "stack"}
												{@const stackTarget = lock.target as { type: "stack"; subject: string }}
												{@const branchesQuery = stackService.branches(projectId, stackTarget.subject)}
												{@const branch = branchesQuery.response}
												{@const commitBranch = branch?.find((b) =>
													b.commits.some((c) => c.id === lock.commitId),
												)}
												{@const knownStack = stackService.allStackById(projectId, stackTarget.subject).response}
												{@const stackHeadBranch = knownStack?.heads.at(0)?.name}
												{@const branchName = commitBranch?.name || knownStack?.heads.at(0)?.name || "Unknown stack"}
												{@const commitMessage = commitBranch?.commits.find(
													(c) => c.id === lock.commitId,
												)}
												{@const commitTitle =
													commitMessage?.message.split("\n")[0] || "No commit message provided"}
												<p class="text-body commit-failed__file-entry-dependency-lock">
													<i class="commit-failed__text-icon"><Icon name="branch-small" /></i>
													<span class="text-semibold">{branchName}</span>
													<i class="clr-text-2">in commit</i>
													<i class="commit-failed__text-icon"><Icon name="commit" /></i>
													<Tooltip text={commitTitle}>
														<span class="commit-failed__tooltip-text text-semibold h-dotted-underline">{lock.commitId.substring(0, 7)}</span>
													</Tooltip>
												</p>

												{#if !commitBranch && knownStack && stackHeadBranch}
													<p class="text-12 clr-text-2">
														This stack exists but is not currently applied in your workspace.
													</p>
													<div class="commit-failed__lock-actions">
														<AsyncButton
															kind="outline"
															loading={applyingStackId === stackTarget.subject}
															action={async () => await applyKnownButUnappliedStack(stackTarget.subject, stackHeadBranch)}
														>
															Apply stack to workspace
														</AsyncButton>
														<Button kind="outline" onclick={() => goto(branchesPath(projectId))}>Open Branches</Button>
														<Button kind="ghost" onclick={() => ignoreLock(lock)}>Ignore for now</Button>
													</div>
												{:else if !commitBranch && !knownStack}
													<p class="text-12 clr-text-2">
														The stack id is no longer known. This can happen when the source stack was removed or rewritten.
													</p>
													<div class="commit-failed__lock-actions">
														<AsyncButton
															kind="outline"
															loading={locatingCommitId === lock.commitId}
															action={async () => await recoverStackByCommit(lock.commitId)}
														>
															Try recover by commit id
														</AsyncButton>
														<Button kind="outline" onclick={() => goto(branchesPath(projectId))}>Open Branches</Button>
														<Button kind="ghost" onclick={() => ignoreLock(lock)}>Ignore for now</Button>
													</div>
												{/if}
											{:else}
												<p class="text-body commit-failed__file-entry-dependency-lock">
													<i class="commit-failed__text-icon"><Icon name="branch-small" /></i>
													<span class="text-semibold">Unknown stack</span>
													<i class="clr-text-2">in commit</i>
													<i class="commit-failed__text-icon"><Icon name="commit" /></i>
													<span class="text-semibold">{lock.commitId.substring(0, 7)}</span>
												</p>
												<p class="text-12 clr-text-2">
													The dependency solver could not map this lock to a stack id in the current workspace.
												</p>
												<div class="commit-failed__lock-actions">
													<AsyncButton
														kind="outline"
														loading={locatingCommitId === lock.commitId}
														action={async () => await recoverStackByCommit(lock.commitId)}
													>
														Try recover by commit id
													</AsyncButton>
													<Button kind="outline" onclick={() => goto(branchesPath(projectId))}>Open Branches</Button>
													<Button kind="ghost" onclick={() => ignoreLock(lock)}>Ignore for now</Button>
												</div>
											{/if}
										{/if}
									{/each}
								</div>
							</div>

							{#if i < fileDependencies.dependencies.length - 1}
								<hr class="commit-failed__file-entry-divider" />
							{/if}
						{/each}
					{/snippet}
				</ReduxResult>
			</div>
		{/if}
	</div>
{:else}
	<div class="commit-failed__file-entry">
		<div class="commit-failed__file-entry__header">
			<FileName filePath={path} textSize="13" />
		</div>
	</div>
{/if}

<style lang="postcss">
	.commit-failed__file-entry__header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px 10px;
		gap: 8px;

		&:hover {
			.commit-failed__file-entry__header__unfold-action {
				opacity: 1;
			}
		}
	}

	.commit-failed__file-entry__header__unfold-action {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--clr-text-2);
		text-wrap: nowrap;
		opacity: 0.7;
		transition: opacity var(--transition-fast);
	}

	.commit-failed__text-icon {
		display: inline-flex;
		align-items: center;
		margin-right: 1px;
		gap: 4px;
		transform: translateY(4px);
		color: var(--clr-text-2);
	}

	.commit-failed__file-entry-dependencies {
		display: flex;
		flex-direction: column;
		padding: 0 12px 12px;
		gap: 10px;
	}

	/* File entry */
	.commit-failed__file-entry {
		display: flex;
		flex-direction: column;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.commit-failed__file-entry-divider {
		margin: 6px -12px;
		border: 0;
		border-top: 1px solid var(--clr-border-2);
	}

	.commit-failed__file-entry__dependency-locks {
		display: flex;
		flex-direction: column;
		padding: 12px;
		gap: 5px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.commit-failed__file-entry__dependency-locks__label {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--clr-theme-warn-element);
	}

	.commit-failed__file-entry__dependency-locks__content {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.commit-failed__lock-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
		padding-top: 2px;
	}

	/* MODIFIERS */
	.clickable {
		cursor: pointer;
	}
</style>
