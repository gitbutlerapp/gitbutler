<script lang="ts">
	import Chrome from '$components/Chrome.svelte';
	import DecorativeSplitView from '$components/DecorativeSplitView.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import directionDoubtSvg from '$lib/assets/illustrations/direction-doubt.svg?raw';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { MODE_SERVICE } from '$lib/mode/modeService';
	import { isNewProjectSettingsPath } from '$lib/routes/routes.svelte';
	import { WORKTREE_SERVICE } from '$lib/worktree/worktreeService.svelte';
	import { inject } from '@gitbutler/core/context';
	import { AsyncButton, RadioButton, FileListItem, Link } from '@gitbutler/ui';
	import type { BaseBranch } from '$lib/baseBranch/baseBranch';
	import type { Snippet } from 'svelte';

	type OptionsType = 'stash' | 'bring-to-workspace';

	interface Props {
		projectId: string;
		baseBranch: BaseBranch;
		children?: Snippet;
	}

	const { projectId, baseBranch, children }: Props = $props();

	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const [setBaseBranchTarget, targetBranchSwitch] = baseBranchService.setTarget;

	const modeService = inject(MODE_SERVICE);
	const mode = $derived(modeService.mode({ projectId }));

	const worktreeService = inject(WORKTREE_SERVICE);
	const changes = $derived(worktreeService.treeChanges(projectId));

	async function switchTarget(branch: string, remote?: string, stashUncommitted?: boolean) {
		await setBaseBranchTarget({
			projectId,
			branch,
			pushRemote: remote,
			stashUncommitted
		});
	}

	const conflicts = $derived(
		mode.current.data?.type === 'OutsideWorkspace' &&
			mode.current.data.subject.worktreeConflicts.length > 0
	);

	let selectedHandlingOfUncommitted: OptionsType = $state('stash');
	let doStash = $derived(selectedHandlingOfUncommitted === 'stash');

	let handlingOptions: { label: string; value: OptionsType; selectable: boolean }[] = $derived([
		{
			label: 'Stash',
			value: 'stash',
			selectable: true
		},
		{
			label: 'Bring to Workspace',
			value: 'bring-to-workspace',
			selectable: !conflicts // TODO: Reactivity??
		}
	]);

	async function initSwithToWorkspace() {
		if (changes.current.data?.length === 0) {
			switchTarget(baseBranch.branchName);
		} else {
			switchTarget(baseBranch.branchName, undefined, doStash);
		}
	}
</script>

<Chrome {projectId} sidebarDisabled>
	{#if children && isNewProjectSettingsPath()}
		<!-- Allow the display of the project settings -->
		{@render children()}
	{:else}
		<DecorativeSplitView img={directionDoubtSvg} hideDetails>
			{@const uncommittedChanges = changes.current.data || []}

			<div class="switchrepo__content">
				<p class="switchrepo__title text-18 text-body text-bold">
					You've switched away from <span class="code-string"> gitbutler/workspace </span>
				</p>

				<p class="switchrepo__message text-13 text-body">
					Due to GitButler managing multiple virtual branches, you cannot switch back and forth
					between git branches and virtual branches easily.
					<Link href="https://docs.gitbutler.com/features/virtual-branches/integration-branch">
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
								{#each uncommittedChanges as change}
									<FileListItem
										filePath={change.path}
										clickable={false}
										hideBorder={change === uncommittedChanges[uncommittedChanges.length - 1]}
									/>
								{/each}
							</div>
							{#if conflicts}
								<p class="switchrepo__label text-13 text-body clr-text-2">
									Some files can’t be applied due to conflicts:
								</p>
								<div class="switchrepo__file-list">
									<ReduxResult result={mode.current} {projectId}>
										{#snippet children(mode, _env)}
											{#if mode.type === 'OutsideWorkspace'}
												{#each mode.subject.worktreeConflicts || [] as path}
													<FileListItem
														filePath={path}
														clickable={false}
														conflicted
														conflictHint="Resolve to apply"
														hideBorder={path ===
															mode.subject.worktreeConflicts[
																mode.subject.worktreeConflicts.length - 1
															]}
													/>
												{/each}
											{/if}
										{/snippet}
									</ReduxResult>
								</div>
							{/if}
						</div>

						<hr class="switchrepo__divider" />

						<p class="switchrepo__label text-13 text-body text-bold">
							What should we do with your uncommitted changes?
						</p>

						<div class="switchrepo__handling-options">
							{#each handlingOptions as item (item.value)}
								<label for={item.value} class="switchrepo__handling-options__item">
									<RadioButton
										id={item.value}
										value={item.value}
										onchange={() => {
											selectedHandlingOfUncommitted = item.value as OptionsType;
											doStash = selectedHandlingOfUncommitted === 'stash';
										}}
										checked={selectedHandlingOfUncommitted === item.value}
									/>
									<p class="text-13 text-body">{item.label}</p>
								</label>
							{/each}
						</div>
					</div>
				{/if}

				<div class="switchrepo__actions">
					<AsyncButton
						style="pop"
						icon="undo-small"
						reversedDirection
						loading={targetBranchSwitch.current.isLoading}
						action={initSwithToWorkspace}
					>
						Switch back
					</AsyncButton>
				</div>
			</div>
		</DecorativeSplitView>
	{/if}
</Chrome>

<style lang="postcss">
	.switchrepo__title {
		margin-bottom: 12px;
		color: var(--clr-text-1);
	}

	.switchrepo__message {
		margin-bottom: 20px;
		color: var(--clr-text-2);
	}

	.switchrepo__content {
		display: flex;
		flex-direction: column;
	}

	.switchrepo__uncommited-changes {
		margin-bottom: 20px;
		padding: 16px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.switchrepo__divider {
		margin: 20px -16px;
		border: 0;
		border-top: 1px solid var(--clr-border-2);
	}

	.switchrepo__label {
		margin-bottom: 12px;
	}

	.switchrepo__file-list {
		margin-bottom: 16px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.switchrepo__handling-options {
		display: flex;
		gap: 20px;
	}

	.switchrepo__handling-options__item {
		display: flex;
		align-items: center;
		gap: 8px;
		cursor: pointer;
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
