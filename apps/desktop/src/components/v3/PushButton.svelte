<script lang="ts">
	import {
		stackHasConflicts,
		stackHasUnpushedCommits,
		stackRequiresForcePush
	} from '$lib/stacks/stack';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UserService } from '$lib/user/userService';
	import { intersectionObserver } from '$lib/utils/intersectionObserver';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';

	type Props = {
		projectId: string;
		stackId: string;
		multipleBranches: boolean;
	};

	const { projectId, stackId, multipleBranches }: Props = $props();

	const stackService = getContext(StackService);
	const userService = getContext(UserService);
	const user = userService.user;
	const stackInfoResult = $derived(stackService.stackInfo(projectId, stackId));
	const stackInfo = $derived(stackInfoResult.current.data);
	const branchesResult = $derived(stackService.branches(projectId, stackId));
	const branches = $derived(branchesResult.current.data || []);
	const [pushStack, pushResult] = stackService.pushStack();
	const [publishBranch, publishResult] = stackService.publishBranch;
	let isSticked = $state(true);

	const requiresForce = $derived(stackInfo && stackRequiresForcePush(stackInfo));
	const hasThingsToPush = $derived(stackInfo && stackHasUnpushedCommits(stackInfo));
	const hasConflicts = $derived(stackInfo && stackHasConflicts(stackInfo));

	async function push() {
		if (requiresForce === undefined) return;
		await pushStack({ projectId, stackId, withForce: requiresForce });

		// Update published branches if they have already been published before
		const topPushedBranch = branches.find((branch) => branch.reviewId && branch.archived);
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

<div
	class="push-button"
	class:is-sticked={isSticked}
	use:intersectionObserver={{
		callback: (entry) => (isSticked = !entry?.isIntersecting),
		options: {
			root: null,
			threshold: 1
		}
	}}
>
	<div class="push-button__inner">
		<Button
			style="neutral"
			wide
			{loading}
			disabled={!hasThingsToPush || hasConflicts}
			tooltip={hasConflicts
				? 'In order to push, please resolve any conflicted commits.'
				: undefined}
			onclick={push}
		>
			{requiresForce ? 'Force push' : multipleBranches ? 'Push All' : 'Push'}
		</Button>
	</div>
</div>

<style lang="postcss">
	.push-button {
		z-index: var(--z-lifted);
		position: sticky;
		padding: 8px 0 8px;
		margin-bottom: -9px;
		bottom: -1px;
		transition: padding var(--transition-medium);

		&:after {
			content: '';
			display: block;
			position: absolute;
			bottom: 0;
			left: -14px;
			height: calc(100% + 8px);
			width: calc(100% + 28px);
			z-index: -1;
			background-color: var(--clr-bg-1);
			border-top: 1px solid var(--clr-border-2);

			transform: translateY(10%);
			opacity: 0;
			transition:
				opacity var(--transition-fast),
				transform var(--transition-medium);
		}

		&.is-sticked {
			padding-bottom: 14px;
		}

		&.is-sticked:after {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.push-button__inner {
		/* This is just here so that the disabled button is still opaque */
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
	}
</style>
