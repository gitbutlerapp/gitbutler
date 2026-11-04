<script lang="ts">
	import IllustrationSplitLayout from "$components/shared/IllustrationSplitLayout.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import AppLayout from "$components/views/AppLayout.svelte";
	import directionDoubtSvg from "$lib/assets/illustrations/direction-doubt.svg?raw";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { MODE_SERVICE } from "$lib/mode/modeService";
	import { WORKTREE_SERVICE } from "$lib/worktree/worktreeService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { AsyncButton, FileListItem, Link, TestId } from "@gitbutler/ui-svelte";
	import type { BaseBranch } from "@gitbutler/but-sdk";
	import type { Snippet } from "svelte";

	interface Props {
		projectId: string;
		baseBranch: BaseBranch;
		children?: Snippet;
	}

	const { projectId, children }: Props = $props();
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const [switchBackToWorkspace, targetBranchSwitch] = baseBranchService.switchBackToWorkspace;

	const modeService = inject(MODE_SERVICE);
	const mode = $derived(modeService.mode(projectId));

	const worktreeService = inject(WORKTREE_SERVICE);
	const changes = $derived(worktreeService.treeChanges(projectId));

	const conflicts = $derived(
		mode.response?.type === "OutsideWorkspace" &&
			mode.response.subject.worktreeConflicts.length > 0,
	);

	async function initSwithToWorkspace() {
		await switchBackToWorkspace({ projectId });
	}
</script>

<AppLayout {projectId} sidebarDisabled>
	{#if children}
		<!-- Allow the display of the project settings -->
		{@render children()}
	{:else}
		<IllustrationSplitLayout img={directionDoubtSvg} hideDetails>
			{@const uncommittedChanges = changes.response || []}

			<div class="switchrepo__content" data-testid={TestId.NotOnGitButlerBranchView}>
				<p class="switchrepo__title text-18 text-body text-bold">
					You've switched away from <span class="code-string"> gitbutler/workspace </span>
				</p>

				<p class="switchrepo__message text-13 text-body">
					Due to GitButler managing multiple virtual branches, you cannot switch back and forth
					between git branches and virtual branches easily.
					<Link href="https://docs.gitbutler.com/features/branch-management/integration-branch">
						Learn more
					</Link>
				</p>

				{#if uncommittedChanges.length > 0}
					<div class="switchrepo__uncommited-changes">
						<div class="switchrepo__uncommited-changes__section">
							<p class="switchrepo__label text-13 text-body text-bold">
								You have uncommitted changes:
							</p>
							<div class="switchrepo__file-list">
								{#each uncommittedChanges as change, i}
									<FileListItem
										filePath={change.path}
										clickable={false}
										isLast={i === uncommittedChanges.length - 1}
									/>
								{/each}
							</div>
							{#if conflicts}
								<p class="switchrepo__label text-13 text-body clr-text-2">
									Some files can’t be applied due to conflicts:
								</p>
								<div class="switchrepo__file-list">
									<ReduxResult result={mode.result} {projectId}>
										{#snippet children(mode, _env)}
											{#if mode.type === "OutsideWorkspace"}
												{#each mode.subject.worktreeConflicts || [] as path, i}
													<FileListItem
														filePath={path}
														clickable={false}
														conflicted
														conflictHint="Resolve to apply"
														isLast={i === (mode.subject.worktreeConflicts?.length ?? 0) - 1}
													/>
												{/each}
											{/if}
										{/snippet}
									</ReduxResult>
								</div>
							{/if}
						</div>

						<p class="text-13 text-body">
							Your uncommitted changes will be carried into the workspace. If they conflict,
							switching will stop. Commit or stash them before retrying.
						</p>
					</div>
				{/if}

				<div class="switchrepo__actions">
					<AsyncButton
						style="pop"
						icon="undo"
						reversedDirection
						loading={targetBranchSwitch.current.isLoading}
						action={initSwithToWorkspace}
					>
						Switch back
					</AsyncButton>
				</div>
			</div>
		</IllustrationSplitLayout>
	{/if}
</AppLayout>

<style lang="postcss">
	.switchrepo__title {
		margin-bottom: 12px;
		color: var(--text-1);
	}

	.switchrepo__message {
		margin-bottom: 20px;
		color: var(--text-2);
	}

	.switchrepo__content {
		display: flex;
		flex-direction: column;
	}

	.switchrepo__uncommited-changes {
		margin-bottom: 20px;
		padding: 16px;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-ml);
	}

	.switchrepo__label {
		margin-bottom: 12px;
	}

	.switchrepo__file-list {
		margin-bottom: 16px;
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
	}

	.switchrepo__actions {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		width: 100%;
		padding-bottom: 24px;
		gap: 8px;
	}
</style>
