<script lang="ts">
	import {
		stackHasConflicts,
		stackHasUnpushedCommits,
		stackRequiresForcePush
	} from '$lib/stacks/stack';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Button from '@gitbutler/ui/Button.svelte';
	import Checkbox from '@gitbutler/ui/Checkbox.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';

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

	const stackService = getContext(StackService);
	const userService = getContext(UserService);
	const user = userService.user;
	const stackInfoResult = $derived(stackService.stackInfo(projectId, stackId));
	const stackInfo = $derived(stackInfoResult.current.data);
	const branchesResult = $derived(stackService.branches(projectId, stackId));
	const branches = $derived(branchesResult.current.data || []);
	const [pushStack, pushResult] = stackService.pushStack;
	const [publishBranch, publishResult] = stackService.publishBranch;

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
		await pushStack({ projectId, stackId, withForce: requiresForce, branch: branchName });

		// Update published branches if they have already been published before
		const topPushedBranch = branches.find((branch) => branch.reviewId);
		if (topPushedBranch && $user) {
			await publishBranch({ projectId, stackId, topBranch: topPushedBranch.name, user: $user });
		}
	}

	const loading = $derived(
		pushResult.current.isLoading ||
			stackInfoResult.current.isLoading ||
			publishResult.current.isLoading
	);

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
			<Button style="pop" type="submit" width={90}>Push</Button>
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
