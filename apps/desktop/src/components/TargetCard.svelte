<script lang="ts">
	import DomainButton from './DomainButton.svelte';
	import SyncButton from '$components/SyncButton.svelte';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { Project } from '$lib/project/projects';
	import { getContext } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	interface Props {
		isNavCollapsed: boolean;
	}

	const { isNavCollapsed }: Props = $props();

	const baseBranchService = getContext(BaseBranchService);
	const project = getContext(Project);

	const base = baseBranchService.base;
	const selected = $derived($page.url.href.endsWith('/base'));
	const baseBranchDiverged = $derived(!!$base?.diverged);
	const baseBranchAheadOnly = $derived(
		baseBranchDiverged && !!$base?.divergedBehind?.length === false
	);
	const divergenceTooltip = $derived(
		baseBranchAheadOnly
			? 'Your local target branch is ahead of its upstream'
			: 'Your local target branch has diverged from its upstream'
	);
</script>

<DomainButton
	alignItems="top"
	{isNavCollapsed}
	isSelected={selected}
	tooltipLabel="Target"
	onmousedown={async () => await goto(`/${project.id}/base`)}
>
	{#if isNavCollapsed}
		{#if ($base?.behind || 0) > 0}
			<div class="small-count-badge">
				<span class="text-10 text-bold">{$base?.behind || 0}</span>
			</div>
		{/if}
	{/if}
	<img class="icon" src="/images/domain-icons/trunk.svg" alt="" />

	{#if !isNavCollapsed}
		<div class="content">
			<div class="button-head">
				<Tooltip text="The branch your Workspace branches are based on and merge into.">
					<span class="text-14 text-semibold trunk-label">Target</span>
				</Tooltip>
				{#if ($base?.behind || 0) > 0 && !baseBranchDiverged}
					<Tooltip text="Unmerged upstream commits">
						<Badge>{$base?.behind || 0}</Badge>
					</Tooltip>
				{/if}
				{#if baseBranchDiverged}
					<Tooltip text={divergenceTooltip}>
						<div>
							<Icon
								name={baseBranchAheadOnly ? 'info' : 'warning'}
								color={baseBranchAheadOnly ? undefined : 'warning'}
							/>
						</div>
					</Tooltip>
				{/if}
				<SyncButton />
			</div>
			<div class="base-branch-label">
				{#if $base?.remoteUrl.includes('github.com')}
					<!-- GitHub logo -->
					<svg
						class="base-branch-icon"
						viewBox="0 0 10 10"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
					>
						<path
							fill-rule="evenodd"
							clip-rule="evenodd"
							d="M5 0C2.2375 0 0 2.29409 0 5.12647C0 7.39493 1.43125 9.31095 3.41875 9.99021C3.66875 10.0351 3.7625 9.88127 3.7625 9.7467C3.7625 9.62495 3.75625 9.22124 3.75625 8.7919C2.5 9.02899 2.175 8.4779 2.075 8.18953C2.01875 8.04215 1.775 7.58717 1.5625 7.46542C1.3875 7.3693 1.1375 7.1322 1.55625 7.12579C1.95 7.11938 2.23125 7.49746 2.325 7.65125C2.775 8.42663 3.49375 8.20876 3.78125 8.07419C3.825 7.74097 3.95625 7.51669 4.1 7.38852C2.9875 7.26036 1.825 6.8182 1.825 4.85733C1.825 4.29983 2.01875 3.83844 2.3375 3.47959C2.2875 3.35143 2.1125 2.82597 2.3875 2.12108C2.3875 2.12108 2.80625 1.98651 3.7625 2.64654C4.1625 2.53119 4.5875 2.47352 5.0125 2.47352C5.4375 2.47352 5.8625 2.53119 6.2625 2.64654C7.21875 1.9801 7.6375 2.12108 7.6375 2.12108C7.9125 2.82597 7.7375 3.35143 7.6875 3.47959C8.00625 3.83844 8.2 4.29342 8.2 4.85733C8.2 6.82461 7.03125 7.26036 5.91875 7.38852C6.1 7.54873 6.25625 7.85631 6.25625 8.33692C6.25625 9.02259 6.25 9.57368 6.25 9.7467C6.25 9.88127 6.34375 10.0415 6.59375 9.99021C8.56875 9.31095 10 7.38852 10 5.12647C10 2.29409 7.7625 0 5 0Z"
						/>
					</svg>
				{:else}
					<svg
						class="base-branch-icon"
						viewBox="0 0 10 10"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
					>
						<path
							fill-rule="evenodd"
							clip-rule="evenodd"
							d="M2 0C0.895431 0 0 0.89543 0 2V8C0 9.10457 0.895431 10 2 10H8C9.10457 10 10 9.10457 10 8V2C10 0.895431 9.10457 0 8 0H2ZM3.32321 1.73901V4.24988H5.242C5.93235 4.24988 6.49199 3.69024 6.49199 2.99988V1.73901H7.99199V2.99988C7.99199 4.51867 6.76078 5.74988 5.242 5.74988H3.32321L3.32321 8.26075H1.82321V1.73901H3.32321Z"
						/>
					</svg>
				{/if}
				<span class="text-12">{$base?.branchName}</span>
			</div>
		</div>
	{/if}
</DomainButton>

<style lang="postcss">
	.icon {
		border-radius: var(--radius-s);
		height: 20px;
		width: 20px;
		flex-shrink: 0;
	}
	.content {
		display: flex;
		flex-direction: column;
		gap: 8px;
		overflow: hidden;
	}
	.trunk-label {
		color: var(--clr-text-1);
	}
	.button-head {
		display: flex;
		gap: 6px;
		align-items: center;
		color: var(--clr-scale-ntrl-10);
	}
	.base-branch-label {
		display: flex;
		align-items: center;
		gap: 4px;
		color: var(--clr-scale-ntrl-40);
		overflow: hidden;

		& span {
			overflow: hidden;
			white-space: nowrap;
			text-overflow: ellipsis;
		}
	}
	.base-branch-icon {
		width: 10px;
		height: 10px;
		fill: currentColor;
	}
	.small-count-badge {
		position: absolute;
		top: 10%;
		right: 10%;

		display: flex;
		align-items: center;
		justify-content: center;
		padding: 2px;
		min-width: 14px;
		background-color: var(--clr-theme-err-element);
		color: var(--clr-scale-ntrl-100);
		border-radius: var(--radius-m);
	}
</style>
