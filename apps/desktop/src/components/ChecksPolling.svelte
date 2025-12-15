<script lang="ts">
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { getPollingInterval } from '$lib/forge/shared/progressivePolling';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/core/context';

	import { Badge, TestId, type MessageStyle } from '@gitbutler/ui';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { ComponentColorType } from '@gitbutler/ui/utils/colorTypes';

	type Props = {
		projectId: string;
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

	let { projectId, branchName, isFork, isMerged, hasChecks = $bindable() }: Props = $props();

	const forge = inject(DEFAULT_FORGE_FACTORY);
	const uiState = inject(UI_STATE);

	const checksService = $derived(forge.current.checks);
	let elapsedMs = $state<number>(0);
	let loadedOnce = $state(false);

	const projectState = $derived(uiState.project(projectId));
	const isDone = $derived(!projectState.branchesToPoll.current.includes(branchName));

	// Do not create a checks monitor if pull request is merged or from a fork.
	// For more information about unavailability of check-runs for forked repos,
	// see GitHub docs at:
	// https://docs.github.com/en/rest/checks/runs?apiVersion=2022-11-28#list-check-runs-in-a-check-suite
	const enabled = $derived(!isFork && !isMerged); // Deduplication.

	const pollingInterval = $derived(getPollingInterval(elapsedMs, isDone));

	const checksQuery = $derived(
		enabled
			? checksService?.get(branchName, { subscriptionOptions: { pollingInterval } })
			: undefined
	);

	const loading = $derived(checksQuery?.result.isLoading);

	const checksTagInfo: StatusInfo = $derived.by(() => {
		const checks = checksQuery?.response;
		if (!checksService && isFork) {
			return {
				style: 'gray',
				icon: 'info',
				text: 'No PR checks',
				reducedText: 'No checks',
				tooltip: 'Checks for forked repos only available on the web.'
			};
		}

		if (checksQuery?.result.error) {
			return {
				style: 'danger',
				icon: 'warning-small',
				text: 'Failed to load',
				reducedText: 'Failed'
			};
		}

		if (checks) {
			const style = checks.completed ? (checks.success ? 'success' : 'danger') : 'warning';
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
			return { style: 'gray', icon: 'spinner', text: 'Checks', reducedText: 'Checks' };
		}

		return { style: 'gray', icon: undefined, text: 'No PR checks', reducedText: 'No checks' };
	});

	// Track previous state to detect transitions.
	// This should **not** be a derived, since we want to track the previous state, not the current one.
	let prevIsDone = $state(false);
	let prevChecksStartedAt = $state<string>();

	// Checks have reached a terminal state or there are no checks to monitor
	const shouldStop = $derived(checksQuery?.response?.completed || checksQuery?.response === null);

	$effect(() => {
		// If polling was previously done but now should restart (e.g., after a force push)
		if (prevIsDone && !isDone) {
			loadedOnce = false;
			elapsedMs = 0;
			prevChecksStartedAt = undefined;
		}

		const result = checksQuery?.result;
		const checks = result?.data;

		// Mark as loaded once we start loading again
		if (loading) {
			loadedOnce = true;
		}

		// If checks are completed, we've loaded them at least once and we are not loading anymore.
		// Stop polling
		if (!isDone && loadedOnce && !loading && shouldStop) {
			projectState.branchesToPoll.remove(branchName);
		}

		// Update elapsed time and hasChecks if checks have started
		if (checks?.startedAt && checks.startedAt !== prevChecksStartedAt) {
			const lastUpdatedMs = Date.parse(checks.startedAt);
			elapsedMs = Date.now() - lastUpdatedMs;
			hasChecks = true;
			prevChecksStartedAt = checks.startedAt;
		}

		// Store previous state for next effect run
		prevIsDone = isDone;
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
