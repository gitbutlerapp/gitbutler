<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { writeClipboard } from '$lib/backend/clipboard';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import {
		branchHasConflicts,
		branchHasUnpushedCommits,
		branchRequiresForcePush
	} from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { getBranchNameFromRef } from '$lib/utils/branch';
	import { splitMessage } from '$lib/utils/commitMessage';
	import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import {
		Button,
		Checkbox,
		Modal,
		TestId,
		SimpleCommitRow,
		ScrollableContainer
	} from '@gitbutler/ui';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';

	type Props = {
		projectId: string;
		stackId: string;
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
	const branchDetails = $derived(stackService.branchDetails(projectId, stackId, branchName));
	const projectResult = $derived(projectsService.getProject(projectId));
	const [pushStack, pushResult] = stackService.pushStack;
	const potentialOverwrittenCommits = $derived(
		stackService.upstreamCommits(projectId, stackId, branchName).current.data || []
	);

	let _currentProject = $state<any>(undefined);

	function handleClick(requiresForce: boolean, project?: any) {
		_currentProject = project;

		if (multipleBranches && !isLastBranchInStack && !$doNotShowPushBelowWarning) {
			confirmationModal?.show();
			return;
		}

		push(requiresForce, project?.force_push_protection);
	}

	async function push(requiresForce: boolean, forcePushProtection: boolean) {
		try {
			const pushResult = await pushStack({
				projectId,
				stackId,
				withForce: requiresForce,
				forcePushProtection: forcePushProtection,
				branch: branchName
			});

			const upstreamBranchNames = pushResult.branchToRemote
				.map(([_, refname]) => getBranchNameFromRef(refname, pushResult.remote))
				.filter(isDefined);
			if (upstreamBranchNames.length === 0) return;
			uiState.project(projectId).branchesToPoll.add(...upstreamBranchNames);
		} catch (error: any) {
			if (error?.code === 'errors.git.force_push_protection') {
				forcePushProtectionModal?.show();
				return;
			}
			throw error;
		}
	}

	const loading = $derived(pushResult.current.isLoading);

	function getButtonTooltip(hasThingsToPush: boolean, hasConflicts: boolean): string | undefined {
		if (!hasThingsToPush) {
			return 'No commits to push';
		}
		if (hasConflicts) {
			return 'In order to push, please resolve any conflicted commits.';
		}
		if (multipleBranches && !isLastBranchInStack) {
			return 'Push this and all branches below';
		}

		return undefined;
	}

	const doNotShowPushBelowWarning = persisted<boolean>(false, 'doNotShowPushBelowWarning');
	let confirmationModal = $state<ReturnType<typeof Modal>>();
	let forcePushProtectionModal = $state<ReturnType<typeof Modal>>();
</script>

<ReduxResult {projectId} result={branchDetails.current}>
	{#snippet children(branchDetails)}
		{@const requiresForce = branchRequiresForcePush(branchDetails)}
		{@const hasThingsToPush = branchHasUnpushedCommits(branchDetails)}
		{@const hasConflicts = branchHasConflicts(branchDetails)}

		<ReduxResult {projectId} result={projectResult.current}>
			{#snippet children(project)}
				<Button
					testId={TestId.StackPushButton}
					kind={isFirstBranchInStack ? 'solid' : 'outline'}
					size="tag"
					style="neutral"
					{loading}
					disabled={!hasThingsToPush || hasConflicts}
					tooltip={getButtonTooltip(hasThingsToPush, hasConflicts)}
					onclick={() => handleClick(requiresForce, project)}
					icon={multipleBranches && !isLastBranchInStack ? 'push-below' : 'push'}
				>
					{requiresForce ? 'Force push' : 'Push'}
				</Button>
			{/snippet}
		</ReduxResult>

		<Modal
			title="Push with dependencies"
			width="small"
			bind:this={confirmationModal}
			onSubmit={async (close) => {
				close();
				push(requiresForce, _currentProject?.force_push_protection);
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
						}}>Cancel</Button
					>
					<Button testId={TestId.StackConfirmPushModalButton} style="pop" type="submit" width={90}
						>Push</Button
					>
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
				push(requiresForce, false);
			}}
		>
			<p class="description">
				Your force push was blocked because the remote branch contains <span
					class="text-bold text-nowrap"
					>{potentialOverwrittenCommits.length === 1
						? '1 commit'
						: `${potentialOverwrittenCommits.length} commits`}</span
				>
				your local branch doesn’t include. To prevent overwriting history,
				<span class="text-bold">cancel and pull & integrate</span> the changes.
			</p>
			<div class="scroll-wrap">
				<ScrollableContainer maxHeight="16.5rem">
					{#each potentialOverwrittenCommits as commit}
						{@const commitUrl = forge.current.commitUrl(commit.id)}
						<SimpleCommitRow
							title={splitMessage(commit.message).title ?? ''}
							sha={commit.id}
							date={new Date(commit.createdAt)}
							author={commit.author.name}
							url={commitUrl}
							onOpen={(url) => openExternalUrl(url)}
							onCopy={() => writeClipboard(commit.id)}
						/>
					{/each}
				</ScrollableContainer>
			</div>

			{#snippet controls(close)}
				<div class="controls">
					<Button kind="outline" type="submit">Force push anyway</Button>
					<Button wide style="pop" onclick={close}>Cancel</Button>
				</div>
			{/snippet}
		</Modal>
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
