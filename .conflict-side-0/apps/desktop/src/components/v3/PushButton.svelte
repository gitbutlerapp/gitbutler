<script lang="ts">
	import {
		stackHasConflicts,
		stackHasUnpushedCommits,
		stackRequiresForcePush
	} from '$lib/stacks/stack';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';

	type Props = {
		projectId: string;
		stackId: string;
		flex?: string;
		multipleBranches: boolean;
	};

	const { projectId, stackId, flex, multipleBranches }: Props = $props();

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

	async function push() {
		if (requiresForce === undefined) return;
		await pushStack({ projectId, stackId, withForce: requiresForce });

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
</script>

<div class="push-button" class:use-flex={!flex} style:flex>
	<Button
		style="neutral"
		wide
		{loading}
		disabled={!hasThingsToPush || hasConflicts}
		tooltip={hasConflicts ? 'In order to push, please resolve any conflicted commits.' : undefined}
		onclick={push}
	>
		{requiresForce ? 'Force push' : multipleBranches ? 'Push all' : 'Push'}
	</Button>
</div>

<style lang="postcss">
	.push-button {
		/* This is just here so that the disabled button is still opaque */
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
	}

	.use-flex {
		flex: 1;
	}
</style>
