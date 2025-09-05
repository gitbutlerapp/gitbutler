<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { DEPENDENCY_SERVICE } from '$lib/dependencies/dependencyService.svelte';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { type RejectionReason } from '$lib/stacks/stackService.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';
	import { FileName, HunkDiff, Icon, Tooltip } from '@gitbutler/ui';

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

	function reasonRelatedToDependencyInfo(reason: RejectionReason): boolean {
		return reason === 'cherryPickMergeConflict' || reason === 'workspaceMergeConflict';
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
					>{isFolded ? 'Show' : 'Hide'}
					hunks ({#if fileDependencies.current.data}
						{fileDependencies.current.data.dependencies.length}
					{:else}
						0
					{/if})</span
				>

				<Icon name={isFolded ? 'chevron-down' : 'chevron-up'} />
			</div>
		</button>

		{#if !isFolded}
			<div class="commit-failed__file-entry-dependencies">
				<ReduxResult {projectId} result={fileDependencies.current}>
					{#snippet children(fileDependencies)}
						{#each fileDependencies.dependencies as dependency, i}
							<HunkDiff
								filePath="test.tsx"
								tabSize={$userSettings.tabSize}
								wrapText={$userSettings.wrapText}
								diffFont={$userSettings.diffFont}
								diffLigatures={$userSettings.diffLigatures}
								diffContrast={$userSettings.diffContrast}
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
										{@const brnachesResult = stackService.branches(projectId, lock.stackId)}
										{@const branch = brnachesResult.current.data}
										{@const commitBranch = branch?.find((b) =>
											b.commits.some((c) => c.id === lock.commitId)
										)}
										{@const branchName = commitBranch?.name || 'Unknown branch'}
										{@const commitMessage = commitBranch?.commits.find(
											(c) => c.id === lock.commitId
										)}
										{@const commitTitle =
											commitMessage?.message.split('\n')[0] || 'No commit message provided'}
										<p class="text-body commit-failed__file-entry-dependency-lock">
											<i class="commit-failed__text-icon"><Icon name="branch-small" /></i>
											<span class="text-semibold">{branchName}</span>
											<i class="clr-text-2">in commit</i>
											<i class="commit-failed__text-icon"><Icon name="commit" /></i>
											<Tooltip text={commitTitle}>
												<span class="commit-failed__tooltip-text text-semibold h-dotted-underline"
													>{lock.commitId.substring(0, 7)}</span
												>
											</Tooltip>
										</p>
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

	/* MODIFIERS */
	.clickable {
		cursor: pointer;
	}
</style>
