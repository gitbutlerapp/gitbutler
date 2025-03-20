<script lang="ts">
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
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
		icon: keyof typeof iconsJson | undefined;
		style?: ComponentColorType;
		messageStyle?: MessageStyle;
		tooltip?: string;
	};

	let { branchName, isFork, isMerged, hasChecks = $bindable() }: Props = $props();

	const [forge] = inject(DefaultForgeFactory);

	const checksService = $derived(forge.current.checks);
	let lastUpdatedStr: string | undefined = $state();
	let stop = $state(false);

	// Do not create a checks monitor if pull request is merged or from a fork.
	// For more information about unavailability of check-runs for forked repos,
	// see GitHub docs at:
	// https://docs.github.com/en/rest/checks/runs?apiVersion=2022-11-28#list-check-runs-in-a-check-suite
	const enabled = $derived(!isFork && !isMerged); // Deduplication.

	let pollingInterval = $derived.by(() => {
		if (!lastUpdatedStr || !enabled || stop) {
			return 0; // Means off.
		}
		const lastUpdatedMs = Date.parse(lastUpdatedStr);
		const elapsedMs = Date.now() - lastUpdatedMs;
		if (elapsedMs < 60 * 1000) {
			return 5 * 1000;
		} else if (elapsedMs < 10 * 60 * 1000) {
			return 30 * 1000;
		} else if (elapsedMs < 60 * 60 * 1000) {
			return 5 * 60 * 1000;
		}
		return 30 * 60 * 1000;
	});

	const result = $derived(
		enabled
			? checksService?.get(branchName, { subscriptionOptions: { pollingInterval } })
			: undefined
	);
	const status = $derived(result?.current.data);
	const loading = $derived(result?.current.isLoading);

	const checksTagInfo: StatusInfo = $derived.by(() => {
		if (!checksService && isFork) {
			return {
				style: 'neutral',
				icon: 'info',
				text: 'No PR checks',
				tooltip: 'Checks for forked repos only available on the web.'
			};
		}

		if (result?.current.error) {
			return { style: 'error', icon: 'warning-small', text: 'Failed to load' };
		}

		if (status) {
			const style = status.completed ? (status.success ? 'success' : 'error') : 'warning';
			const icon =
				status.completed && !loading
					? status.success
						? 'success-small'
						: 'error-small'
					: 'spinner';
			const text = status.completed
				? status.success
					? 'Checks passed'
					: 'Checks failed'
				: 'Checks running';
			return { style, icon, text };
		}
		if (loading) {
			return { style: 'neutral', icon: 'spinner', text: 'Checks' };
		}

		return { style: 'neutral', icon: undefined, text: 'No PR checks' };
	});

	$effect(() => {
		if (!status) return;
		const { startedAt, success } = status;
		if (startedAt) {
			hasChecks = true;
			lastUpdatedStr = startedAt;
		}
		if (success) {
			stop = true;
		}
	});
</script>

<Badge
	size="tag"
	icon={checksTagInfo.icon}
	style={checksTagInfo.style}
	kind={checksTagInfo.icon === 'success-small' ? 'solid' : 'soft'}
	tooltip={checksTagInfo.tooltip}
>
	{checksTagInfo.text}
</Badge>
