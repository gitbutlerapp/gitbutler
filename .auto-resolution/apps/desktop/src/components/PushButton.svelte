<script lang="ts">
	import GerritPushModal from '$components/GerritPushModal.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { CLIPBOARD_SERVICE } from '$lib/backend/clipboard';
	import { projectRunCommitHooks } from '$lib/config/config';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import {
		branchHasConflicts,
		branchHasUnpushedCommits,
		partialStackRequestsForcePush
	} from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { getBranchNameFromRef } from '$lib/utils/branch';
	import { splitMessage } from '$lib/utils/commitMessage';
	import { URL_SERVICE } from '$lib/utils/url';
	import { inject } from '@gitbutler/core/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import {
		Button,
		Checkbox,
		Modal,
		TestId,
		SimpleCommitRow,
		ScrollableContainer,
		chipToasts
	} from '@gitbutler/ui';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import type { GerritPushFlag } from '$lib/stacks/stack';

	type Props = {
		projectId: string;
		stackId?: string;
		branchName: string;
		multipleBranches: boolean;
		isLastBranchInStack?: boolean;
		isFirstBranchInStack?: boolean;
	};

	const {
		projectId,
		branchName,
		stackId,
		multipleBranches,
		isFirstBranchInStack,
		isLastBranchInStack
	}: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const projectsService = inject(PROJECTS_SERVICE);
	const uiState = inject(UI_STATE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const urlService = inject(URL_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);

	// Get current project to check gerrit_mode
	const projectsQuery = $derived(projectsService.projects());
	const projects = $derived(projectsQuery.response);
	const currentProject = $derived(projects?.find((p) => p.id === projectId));
	const isGerritMode = $derived(currentProject?.gerrit_mode ?? false);

	// Component is read-only when stackId is undefined
	const isReadOnly = $derived(!stackId);

	const branchDetails = $derived(stackService.branchDetails(projectId, stackId, branchName));
	const branchesQuery = $derived(stackService.branches(projectId, stackId));
	const [pushStack, pushQuery] = stackService.pushStack;
	const upstreamCommitsQuery = $derived(
		stackService.upstreamCommits(projectId, stackId, branchName)
	);
	const upstreamCommits = $derived(upstreamCommitsQuery.response);
	const runHooks = $derived(projectRunCommitHooks(projectId));

	function handleClick(args: { withForce: boolean; skipForcePushProtection: boolean }) {
		if (isGerritMode) {
			gerritModal?.show();
			return;
		}

		if (multipleBranches && !isLastBranchInStack && !$doNotShowPushBelowWarning) {
			confirmationModal?.show();
			return;
		}

		push(args);
	}

	async function push(args: {
		withForce: boolean;
		skipForcePushProtection: boolean;
		gerritFlag?: GerritPushFlag;
	}) {
		const { withForce, skipForcePushProtection, gerritFlag } = args;
		try {
			const pushResult = await pushStack({
				projectId,
				stackId,
				withForce,
				skipForcePushProtection,
				branch: branchName,
				runHooks: $runHooks,
				gerritFlag
			});

			const upstreamBranchNames = pushResult.branchToRemote
				.map(([_, refname]) => getBranchNameFromRef(refname, pushResult.remote))
				.filter(isDefined);
			if (upstreamBranchNames.length === 0) return;
			uiState.project(projectId).branchesToPoll.add(...upstreamBranchNames);

			// Show success notification
			const branchText =
				multipleBranches && !isLastBranchInStack
					? `${branchName} and all branches below it`
					: branchName;
			chipToasts.success(`Pushed ${branchText} successfully`);
		} catch (error: any) {
			if (error?.code === 'errors.git.force_push_protection') {
				forcePushProtectionModal?.show();
				return;
			}
			throw error;
		}
	}

	const loading = $derived(pushQuery.current.isLoading);

	function getButtonTooltip(
		hasThingsToPush: boolean,
		hasConflicts: boolean,
		withForce: boolean,
		remoteTrackingBranch: string | null
	): string | undefined {
		if (isReadOnly) {
			return 'Read-only mode';
		}

		if (!hasThingsToPush) {
			return 'No commits to push';
		}

		if (hasConflicts) {
			return 'In order to push, please resolve any conflicted commits.';
		}

		if (multipleBranches && !isLastBranchInStack) {
			return 'Push this and all branches below';
		}

		if (withForce) {
			return remoteTrackingBranch
				? 'Force push this branch'
				: `Force push this branch to ${remoteTrackingBranch}`;
		}

		return remoteTrackingBranch
			? `Push this branch to ${remoteTrackingBranch}`
			: 'Push this branch';
	}

	const doNotShowPushBelowWarning = persisted<boolean>(false, 'doNotShowPushBelowWarning');
	let confirmationModal = $state<ReturnType<typeof Modal>>();
	let forcePushProtectionModal = $state<ReturnType<typeof Modal>>();
	let gerritModal = $state<GerritPushModal>();
	let pendingGerritFlag = $state<GerritPushFlag | undefined>();
</script>

<ReduxResult {projectId} result={combineResults(branchDetails.result, branchesQuery.result)}>
	{#snippet children([branchDetails, branches])}
		{@const withForce = partialStackRequestsForcePush(branchName, branches)}
		{@const hasThingsToPush = branchHasUnpushedCommits(branchDetails)}
		{@const hasConflicts = branchHasConflicts(branchDetails)}
		{@const buttonDisabled = isReadOnly || !hasThingsToPush || hasConflicts}

		<Button
			testId={TestId.StackPushButton}
			kind={isFirstBranchInStack ? 'solid' : 'outline'}
			size="tag"
			style="neutral"
			{loading}
			disabled={buttonDisabled}
			tooltip={getButtonTooltip(
				hasThingsToPush,
				hasConflicts,
				withForce,
				branchDetails.remoteTrackingBranch
			)}
			onclick={() => handleClick({ withForce, skipForcePushProtection: false })}
			icon={multipleBranches && !isLastBranchInStack ? 'push-below' : 'push'}
		>
			{isGerritMode ? 'Push' : withForce ? 'Force push' : 'Push'}
		</Button>

		<Modal
			title="Push with dependencies"
			width="small"
			bind:this={confirmationModal}
			onSubmit={async (close) => {
				close();
				push({ withForce, skipForcePushProtection: false, gerritFlag: pendingGerritFlag });
				pendingGerritFlag = undefined;
			}}
		>
			<p>
				You're about to push <span class="text-bold">{branchName}</span>. To maintain the correct
				history, GitButler will also push all branches below this branch in the stack.
			</p>

			{#snippet controls(close)}
				<div class="modal-footer">
					<div class="flex flex-1">
						<label for="dont-show-again" class="modal-footer__checkbox">
							<Checkbox name="dont-show-again" small bind:checked={$doNotShowPushBelowWarning} />
							<span class="text-12"> Don’t show again</span>
						</label>
					</div>
					<Button
						kind="outline"
						onclick={() => {
							$doNotShowPushBelowWarning = false;
							close();
						}}
					>
						Cancel
					</Button>
					<Button testId={TestId.StackConfirmPushModalButton} style="pop" type="submit" width={90}>
						Push
					</Button>
				</div>
			{/snippet}
		</Modal>

		<Modal
			title="Protected force push"
			width={480}
			type="warning"
			bind:this={forcePushProtectionModal}
			onSubmit={async (close) => {
				close();
				push({ withForce, skipForcePushProtection: true, gerritFlag: pendingGerritFlag });
				pendingGerritFlag = undefined;
			}}
		>
			<p class="description">
				Your force push was blocked because the remote branch contains <span
					class="text-bold text-nowrap"
					>{upstreamCommits?.length === 1 ? '1 commit' : `${upstreamCommits?.length} commits`}</span
				>
				your local branch doesn’t include. To prevent overwriting history,
				<span class="text-bold">cancel and pull & integrate</span> the changes.
			</p>
			{#if upstreamCommits}
				<div class="scroll-wrap">
					<ScrollableContainer maxHeight="16.5rem">
						{#each upstreamCommits as commit}
							{@const commitUrl = forge.current.commitUrl(commit.id)}
							<SimpleCommitRow
								title={splitMessage(commit.message).title ?? ''}
								sha={commit.id}
								date={new Date(commit.createdAt)}
								author={commit.author.name}
								url={commitUrl}
								onOpen={(url) => urlService.openExternalUrl(url)}
								onCopy={() => clipboardService.write(commit.id, { message: 'Commit hash copied' })}
							/>
						{/each}
					</ScrollableContainer>
				</div>
			{/if}

			{#snippet controls(close)}
				<div class="controls">
					<Button kind="outline" type="submit">Force push anyway</Button>
					<Button wide style="pop" onclick={close}>Cancel</Button>
				</div>
			{/snippet}
		</Modal>

		<GerritPushModal
			bind:this={gerritModal}
			{projectId}
			{stackId}
			{branchName}
			{multipleBranches}
			{isFirstBranchInStack}
			{isLastBranchInStack}
			onPush={(gerritFlag) => {
				if (multipleBranches && !isLastBranchInStack && !$doNotShowPushBelowWarning) {
					// Store the gerrit flag for later use when confirmation modal completes
					pendingGerritFlag = gerritFlag;
					confirmationModal?.show();
				} else {
					push({ withForce, skipForcePushProtection: false, gerritFlag });
				}
			}}
		/>
	{/snippet}
</ReduxResult>

<style>
	/* MODAL */
	.modal-footer {
		display: flex;
		width: 100%;
		gap: 6px;
	}

	/* CONTROLS */
	.controls {
		display: flex;
		width: 100%;
		gap: 6px;
	}

	.modal-footer__checkbox {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	/* COMMITS SCROLL CONTAINER */
	.description {
		margin: 0 0 16px;
	}
	.scroll-wrap {
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}
</style>
