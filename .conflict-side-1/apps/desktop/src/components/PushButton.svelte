<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import {
		branchHasConflicts,
		branchHasUnpushedCommits,
		branchRequiresForcePush
	} from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { getBranchNameFromRef } from '$lib/utils/branch';
	import { inject } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import { Button, Checkbox, Modal, TestId } from '@gitbutler/ui';
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
	const uiState = inject(UI_STATE);
	const branchDetails = $derived(stackService.branchDetails(projectId, stackId, branchName));
	const [pushStack, pushResult] = stackService.pushStack;

	function handleClick(requiresForce: boolean) {
		if (multipleBranches && !isLastBranchInStack && !$doNotShowPushBelowWarning) {
			confirmationModal?.show();
			return;
		}

		push(requiresForce);
	}

	async function push(requiresForce: boolean) {
		const pushResult = await pushStack({
			projectId,
			stackId,
			withForce: requiresForce,
			branch: branchName
		});

		const upstreamBranchNames = pushResult.branchToRemote
			.map(([_, refname]) => getBranchNameFromRef(refname, pushResult.remote))
			.filter(isDefined);
		if (upstreamBranchNames.length === 0) return;
		uiState.project(projectId).branchesToPoll.add(...upstreamBranchNames);
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
</script>

<ReduxResult {projectId} result={branchDetails.current}>
	{#snippet children(branchDetails)}
		{@const requiresForce = branchRequiresForcePush(branchDetails)}
		{@const hasThingsToPush = branchHasUnpushedCommits(branchDetails)}
		{@const hasConflicts = branchHasConflicts(branchDetails)}
		<Button
			testId={TestId.StackPushButton}
			kind={isFirstBranchInStack ? 'solid' : 'outline'}
			size="tag"
			style="neutral"
			{loading}
			disabled={!hasThingsToPush || hasConflicts}
			tooltip={getButtonTooltip(hasThingsToPush, hasConflicts)}
			onclick={() => handleClick(requiresForce)}
			icon={multipleBranches && !isLastBranchInStack ? 'push-below' : 'push'}
		>
			{requiresForce ? 'Force push' : 'Push'}
		</Button>

		<Modal
			title="Push with dependencies"
			width="small"
			bind:this={confirmationModal}
			onSubmit={async (close) => {
				close();
				push(requiresForce);
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
	{/snippet}
</ReduxResult>

<style>
	/* MODAL */
	.modal-footer {
		display: flex;
		width: 100%;
		gap: 6px;
	}

	.modal-footer__checkbox {
		display: flex;
		align-items: center;
		gap: 8px;
	}
</style>
