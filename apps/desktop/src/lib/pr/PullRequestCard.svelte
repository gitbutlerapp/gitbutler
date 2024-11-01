<script lang="ts">
	import MergeButton from './MergeButton.svelte';
	import { Project } from '$lib/backend/projects';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { getForgeChecksMonitor } from '$lib/forge/interface/forgeChecksMonitor';
	import { getForgeListingService } from '$lib/forge/interface/forgeListingService';
	import { getForgePrService } from '$lib/forge/interface/forgePrService';
	import * as toasts from '$lib/utils/toasts';
	import { openExternalUrl } from '$lib/utils/url';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { type ComponentColor } from '@gitbutler/ui/utils/colorTypes';
	import type { MessageStyle } from '$lib/shared/InfoMessage.svelte';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	interface Props {
		upstreamName: string;
	}

	const { upstreamName }: Props = $props();

	type StatusInfo = {
		text: string;
		icon?: keyof typeof iconsJson | undefined;
		style?: ComponentColor;
		messageStyle?: MessageStyle;
	};

	const vbranchService = getContext(VirtualBranchService);
	const baseBranchService = getContext(BaseBranchService);
	const project = getContext(Project);

	const forgeListingService = getForgeListingService();
	const prStore = $derived($forgeListingService?.prs);
	const prs = $derived(prStore ? $prStore : undefined);

	const listedPr = $derived(prs?.find((pr) => pr.sourceBranch === upstreamName));
	const prNumber = $derived(listedPr?.number);

	const prService = getForgePrService();
	const prMonitor = $derived(prNumber ? $prService?.prMonitor(prNumber) : undefined);

	// This PR has been loaded on demand, and contains more details than the version
	// obtained when listing them.
	const pr = $derived(prMonitor?.pr);

	const checksMonitor = getForgeChecksMonitor();
	const checks = $derived($checksMonitor?.status);

	// While the pr monitor is set to fetch updates by interval, we want
	// frequent updates while checks are running.
	$effect(() => {
		if ($checks) prMonitor?.refresh();
	});

	let isMerging = $state(false);

	const mrLoading = $derived(prMonitor?.loading);
	const checksLoading = $derived($checksMonitor?.loading);

	const checksError = $derived($checksMonitor?.error);
	const detailsError = $derived(prMonitor?.error);

	const checksTagInfo: StatusInfo = $derived.by(() => {
		if ($checksError || $detailsError) {
			return { style: 'error', icon: 'warning-small', text: 'Failed to load' };
		}

		if ($checks) {
			const style = $checks.completed ? ($checks.success ? 'success' : 'error') : 'warning';
			const icon =
				$checks.completed && !$checksLoading
					? $checks.success
						? 'success-small'
						: 'error-small'
					: 'spinner';
			const text = $checks.completed ? 'Checks' : 'Checks running';
			return { style, icon, text };
		}
		if ($checksLoading) {
			return { style: 'neutral', icon: 'spinner', text: 'Checks' };
		}

		return { style: 'neutral', icon: undefined, text: 'No PR checks' };
	});

	const prStatusInfo: StatusInfo = $derived.by(() => {
		if (!$pr) {
			return { text: 'Status', style: 'neutral' };
		}

		if ($pr?.mergedAt) {
			return { text: 'Merged', style: 'purple' };
		}

		if ($pr?.closedAt) {
			return { text: 'Closed', style: 'error' };
		}

		if ($pr?.draft) {
			return { text: 'Draft', style: 'neutral' };
		}

		return { text: 'Open', style: 'success' };
	});
</script>

{#if $pr}
	<div class="card pr-card">
		<div class="pr-title text-13 text-semibold">
			<span style="color: var(--clr-scale-ntrl-50)">PR #{$pr?.number}:</span>
			<span>{$pr.title}</span>
		</div>
		<div class="pr-tags">
			<Button
				reversedDirection
				size="tag"
				clickable={false}
				style={prStatusInfo.style}
				tooltip="PR status"
				kind={'soft'}
			>
				{prStatusInfo.text}
			</Button>
			{#if !$pr.closedAt && checksTagInfo}
				<Button
					size="tag"
					clickable={false}
					icon={checksTagInfo.icon}
					reversedDirection
					style={checksTagInfo.style}
					kind={checksTagInfo.icon === 'success-small' ? 'solid' : 'soft'}
				>
					{checksTagInfo.text}
				</Button>
			{/if}
			<Button
				icon="open-link"
				size="tag"
				style="ghost"
				outline
				tooltip="Open in browser"
				onclick={() => {
					openExternalUrl($pr.htmlUrl);
				}}
			>
				View PR
			</Button>
		</div>

		<!--
        We can't show the merge button until we've waited for checks

        We use a octokit.checks.listForRef to find checks running for a PR, but right after
        creation this request succeeds but returns an empty array. So we need a better way
        determining "no checks will run for this PR" such that we can show the merge button
        immediately.
        -->
		{#if $pr}
			<div class="pr-actions">
				<MergeButton
					wide
					projectId={project.id}
					disabled={$mrLoading ||
						$checksLoading ||
						$pr.draft ||
						!$pr.mergeable ||
						['dirty', 'unknown', 'blocked', 'behind'].includes($pr.mergeableState)}
					loading={isMerging}
					onclick={async (method) => {
						if (!$pr) return;
						isMerging = true;
						try {
							await $prService?.merge(method, $pr.number);
							await baseBranchService.fetchFromRemotes();
							await Promise.all([
								prMonitor?.refresh(),
								$forgeListingService?.refresh(),
								vbranchService.refresh(),
								baseBranchService.refresh()
							]);
						} catch (err) {
							console.error(err);
							toasts.error('Failed to merge pull request');
						} finally {
							isMerging = false;
						}
					}}
				/>
			</div>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.pr-card {
		position: relative;
		padding: 14px;
		margin-bottom: 8px;
	}

	.pr-title {
		color: var(--clr-scale-ntrl-0);
		margin-bottom: 12px;
		margin-right: 28px;
		user-select: text;
		cursor: text;
	}

	.pr-tags {
		display: flex;
		gap: 4px;
	}

	.pr-actions {
		margin-top: 14px;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
</style>
