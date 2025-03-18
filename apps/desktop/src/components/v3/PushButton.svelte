<script lang="ts">
	import {
		stackHasConflicts,
		stackHasUnpushedCommits,
		stackRequiresForcePush
	} from '$lib/stacks/stack';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { intersectionObserver } from '$lib/utils/intersectionObserver';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';

	type Props = {
		projectId: string;
		stackId: string;
		multipleBranches: boolean;
	};

	const { projectId, stackId, multipleBranches }: Props = $props();

	const [stackService] = inject(StackService);
	const stackInfoResult = $derived(stackService.stackInfo(projectId, stackId));
	const stackInfo = $derived(stackInfoResult.current.data);
	const [pushStack, pushResult] = stackService.pushStack();
	let scrollEndVisible = $state(true);

	const requiresForce = $derived(stackInfo && stackRequiresForcePush(stackInfo));
	const hasThingsToPush = $derived(stackInfo && stackHasUnpushedCommits(stackInfo));
	const hasConflicts = $derived(stackInfo && stackHasConflicts(stackInfo));

	function push() {
		if (requiresForce === undefined) return;
		pushStack({ projectId, stackId, withForce: requiresForce });
	}

	const loading = $derived(pushResult.current.isLoading || stackInfoResult.current.isLoading);
</script>

<div
	class="push-button"
	class:scroll-end-visible={scrollEndVisible}
	use:intersectionObserver={{
		callback: (entry) => {
			if (entry?.isIntersecting) {
				scrollEndVisible = false;
			} else {
				scrollEndVisible = true;
			}
		},
		options: {
			root: null,
			rootMargin: `-100% 0px 0px 0px`,
			threshold: 0
		}
	}}
>
	<div class="push-button__inner">
		<Button
			style="pop"
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
		padding: 20px 12px 12px;
		bottom: 0;

		transition: background-color var(--transition-fast);

		&:global(.merge-all > button:not(:last-child)) {
			margin-bottom: 8px;
		}

		&:after {
			content: '';
			display: block;
			position: absolute;
			bottom: 0;
			left: 0;
			height: calc(100% + 12px);
			width: 100%;
			z-index: -1;
			background-color: var(--clr-bg-1);
			border-top: 1px solid var(--clr-border-2);

			transform: translateY(0);
			opacity: 0;
			transition: opacity var(--transition-fast);
		}

		&:not(.scroll-end-visible):after {
			opacity: 1;
		}
	}

	.push-button__inner {
		/* This is just here so that the disabled button is still opaque */
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
	}
</style>
