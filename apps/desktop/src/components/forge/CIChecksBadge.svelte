<script lang="ts">
	import { DEFAULT_FORGE_FACTORY } from "$lib/forge/forgeFactory.svelte";
	import { getPollingInterval } from "$lib/forge/shared/progressivePolling";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";

	import { Badge, TestId, type MessageStyle, type IconName } from "@gitbutler/ui";
	import type { ComponentColorType } from "@gitbutler/ui/utils/colorTypes";

	type Props = {
		projectId: string;
		branchName: string;
		prUpdatedAt?: string;
		hasChecks?: boolean;
		isFork?: boolean;
		isMerged?: boolean;
	};

	type StatusInfo = {
		text: string;
		reducedText: string;
		icon?: IconName;
		style?: ComponentColorType;
		messageStyle?: MessageStyle;
		tooltip?: string;
	};

	let {
		projectId,
		branchName,
		prUpdatedAt,
		isFork,
		isMerged,
		hasChecks = $bindable(),
	}: Props = $props();

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
			: undefined,
	);

	const loading = $derived(checksQuery?.result.isLoading);

	const checksTagInfo: StatusInfo = $derived.by(() => {
		const checks = checksQuery?.response;
		if (!checksService && isFork) {
			return {
				style: "gray",
				icon: undefined,
				text: "No PR checks",
				reducedText: "No checks",
				tooltip: "Checks for forked repos only available on the web.",
			};
		}

		if (checksQuery?.result.error) {
			return {
				style: "danger",
				icon: "warning",
				text: "Failed to load checks",
				reducedText: "Error",
				tooltip: "Failed to load checks. Click to retry.",
			};
		}

		if (checks) {
			const style = checks.completed ? (checks.success ? "safe" : "danger") : "warning";
			// Keep the terminal icon stable during background re-fetches
			const icon = checks.completed ? (checks.success ? "tick" : "danger") : "spinner";
			const text = checks.completed
				? checks.success
					? "Checks passed"
					: "Checks failed"
				: "Checks running";

			const tooltip =
				checks.completed && !checks.success
					? `Checks failed: ${checks.failedChecks.join(", ")}`
					: undefined;

			const reducedText = checks.completed ? (checks.success ? "Passed" : "Failed") : "Running";
			return { style, icon, text, reducedText, tooltip };
		}
		if (loading) {
			return {
				style: "gray",
				icon: "spinner",
				text: "Loading checks",
				reducedText: "Checks",
				tooltip: "Waiting for checks to start…",
			};
		}

		return {
			style: "gray",
			icon: undefined,
			text: "No checks configured",
			reducedText: "No checks",
			tooltip: "No CI checks are configured.",
		};
	});

	// Track previous state to detect transitions.
	// This should **not** be a derived, since we want to track the previous state, not the current one.
	let prevIsDone = $state(false);
	let prevChecksStartedAt = $state<string>();
	let prevPrUpdatedAt = $state<string>();

	// After a PR update (e.g. push), GitHub may still return old completed checks
	// before creating the new check runs. We prevent polling from stopping for a
	// grace period after prUpdatedAt changes, to give GitHub time to catch up.
	const STALE_GRACE_PERIOD_MS = 60_000;
	let prUpdatedAtChangedTime = $state<number>();

	// Checks have reached a terminal state or there are no checks to monitor.
	// Note: shouldStop is computed in the $effect below since the grace period
	// depends on wall-clock time (Date.now()) which isn't reactive.
	let shouldStop = $state(false);

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

		// Compute shouldStop fresh each time the effect runs, since the grace
		// period depends on wall-clock time.
		const withinGracePeriod =
			prUpdatedAtChangedTime !== undefined &&
			Date.now() - prUpdatedAtChangedTime < STALE_GRACE_PERIOD_MS;
		const checksCompleted = checksQuery?.response?.completed || checksQuery?.response === null;
		shouldStop = !withinGracePeriod && checksCompleted;

		if (!isDone && loadedOnce && !loading && shouldStop) {
			projectState.branchesToPoll.remove(branchName);
		}

		// Reset polling frequency when the PR is updated (e.g. after a push).
		if (prUpdatedAt && prUpdatedAt !== prevPrUpdatedAt) {
			const parsed = Date.parse(prUpdatedAt);
			if (!Number.isNaN(parsed)) {
				elapsedMs = Date.now() - parsed;
				prUpdatedAtChangedTime = Date.now();
			}
			prevPrUpdatedAt = prUpdatedAt;
		}

		// Update elapsed time and hasChecks if checks have started
		if (checks?.startedAt && checks.startedAt !== prevChecksStartedAt) {
			const parsed = Date.parse(checks.startedAt);
			if (!Number.isNaN(parsed)) {
				elapsedMs = Date.now() - parsed;
			}
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
	kind={checksTagInfo.icon === "tick" ? "solid" : "soft"}
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
