<script lang="ts">
	import {
		stackHasConflicts,
		stackHasUnpushedCommits,
		stackRequiresForcePush
	} from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { getBranchNameFromRef } from '$lib/utils/branch';
	import { inject } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import { Button, Checkbox, Modal } from '@gitbutler/ui';

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
	const stackInfoResult = $derived(stackService.stackInfo(projectId, stackId));
	const stackInfo = $derived(stackInfoResult.current.data);
	const [pushStack, pushResult] = stackService.pushStack;

	const requiresForce = $derived(stackInfo && stackRequiresForcePush(stackInfo));
	const hasThingsToPush = $derived(stackInfo && stackHasUnpushedCommits(stackInfo));
	const hasConflicts = $derived(stackInfo && stackHasConflicts(stackInfo));

	function handleClick() {
		if (multipleBranches && !isLastBranchInStack && !$doNotShowPushBelowWarning) {
			confirmationModal?.show();
			return;
		}

		push();
	}

	async function push() {
		if (requiresForce === undefined) return;
		const pushResult = await pushStack({
			projectId,
			stackId,
			withForce: requiresForce,
			branch: branchName
		});

		const upstreamBranchName = getBranchNameFromRef(pushResult.refname, pushResult.remote);
		if (upstreamBranchName === undefined) return;
		uiState.project(projectId).branchesToPoll.add(upstreamBranchName);
	}

	const loading = $derived(pushResult.current.isLoading || stackInfoResult.current.isLoading);

	function getButtonTooltip() {
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

<Modal
	title="Push with dependencies"
	width="small"
	bind:this={confirmationModal}
	onSubmit={async (close) => {
		close();
		push();
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

<Button
	testId={TestId.StackPushButton}
	kind={isFirstBranchInStack ? 'solid' : 'outline'}
	size="tag"
	style="neutral"
	{loading}
	disabled={!hasThingsToPush || hasConflicts}
	tooltip={getButtonTooltip()}
	onclick={handleClick}
	icon={multipleBranches && !isLastBranchInStack ? 'push-below' : 'push'}
>
	{requiresForce ? 'Force push' : 'Push'}
</Button>

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
