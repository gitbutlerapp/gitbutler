<script lang="ts">
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { getPollingInterval } from '$lib/forge/shared/progressivePolling';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';

	import { Badge } from '@gitbutler/ui';
	import type { MessageStyle } from '$components/InfoMessage.svelte';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { ComponentColorType } from '@gitbutler/ui/utils/colorTypes';

	type Props = {
		branchName: string;
		hasChecks?: boolean;
		isFork?: boolean;
		isMerged?: boolean;
	};

	type StatusInfo = {
		text: string;
		reducedText: string;
		icon: keyof typeof iconsJson | undefined;
		style?: ComponentColorType;
		messageStyle?: MessageStyle;
		tooltip?: string;
	};

	let { branchName, isFork, isMerged, hasChecks = $bindable() }: Props = $props();

	const forge = inject(DEFAULT_FORGE_FACTORY);

	const checksService = $derived(forge.current.checks);
	let elapsedMs = $state<number>(0);
	let isDone = $state(false);

	// Do not create a checks monitor if pull request is merged or from a fork.
	// For more information about unavailability of check-runs for forked repos,
	// see GitHub docs at:
	// https://docs.github.com/en/rest/checks/runs?apiVersion=2022-11-28#list-check-runs-in-a-check-suite
	const enabled = $derived(!isFork && !isMerged); // Deduplication.

	let pollingInterval = $derived(getPollingInterval(elapsedMs, isDone));

	const checksResult = $derived(
		enabled
			? checksService?.get(branchName, { subscriptionOptions: { pollingInterval } })
			: undefined
	);

	let timeoutId: any = undefined;
	let loading = $state(false);

	$effect(() => {
		if (checksResult?.current.isLoading) {
			timeoutId = setTimeout(() => (loading = true), 500);
		} else {
			if (timeoutId) clearTimeout(timeoutId);
			loading = false;
		}
	});

	const checksTagInfo: StatusInfo = $derived.by(() => {
		const checks = checksResult?.current.data;
		if (!checksService && isFork) {
			return {
				style: 'neutral',
				icon: 'info',
				text: 'No PR checks',
				reducedText: 'No checks',
				tooltip: 'Checks for forked repos only available on the web.'
			};
		}

		if (checksResult?.current.error) {
			return {
				style: 'error',
				icon: 'warning-small',
				text: 'Failed to load',
				reducedText: 'Failed'
			};
		}

		if (checks) {
			const style = checks.completed ? (checks.success ? 'success' : 'error') : 'warning';
			const icon =
				checks.completed && !loading
					? checks.success
						? 'success-small'
						: 'error-small'
					: 'spinner';
			const text = checks.completed
				? checks.success
					? 'Checks passed'
					: 'Checks failed'
				: 'Checks running';

			const tooltip =
				checks.completed && !checks.success
					? `Checks failed: ${checks.failedChecks.join(', ')}`
					: undefined;

			const reducedText = checks.completed ? (checks.success ? 'Passed' : 'Failed') : 'Running';
			return { style, icon, text, reducedText, tooltip };
		}
		if (loading) {
			return { style: 'neutral', icon: 'spinner', text: 'Checks', reducedText: 'Checks' };
		}

		return { style: 'neutral', icon: undefined, text: 'No PR checks', reducedText: 'No checks' };
	});

	$effect(() => {
		const result = checksResult?.current;
		const checks = result?.data;

		if (checks?.completed || checks === null) {
			isDone = true;
			return;
		}

		if (checks?.startedAt) {
			const lastUpdatedMs = Date.parse(checks.startedAt);
			elapsedMs = Date.now() - lastUpdatedMs;
			hasChecks = true;
		}
	});
</script>

<Badge
	testId={TestId.PRChecksBadge}
	size="icon"
	icon={checksTagInfo.icon}
	style={checksTagInfo.style}
	kind={checksTagInfo.icon === 'success-small' ? 'solid' : 'soft'}
	tooltip={checksTagInfo.tooltip}
	reversedDirection
	onclick={(e) => {
		checksService?.fetch(branchName, { forceRefetch: true });
		e.stopPropagation();
	}}
>
	<span data-pr-text={checksTagInfo.reducedText} class="truncate">
		{checksTagInfo.reducedText}
	</span>
</Badge>
