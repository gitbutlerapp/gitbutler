<script lang="ts">
	import DomainButton from './DomainButton.svelte';
	import SyncButton from '../components/SyncButton.svelte';
	import { Project } from '$lib/backend/projects';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { getContext } from '$lib/utils/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	export let isNavCollapsed: boolean;

	const baseBranchService = getContext(BaseBranchService);
	const project = getContext(Project);

	const base = baseBranchService.base;
	$: selected = $page.url.href.endsWith('/base');
	$: baseBranchDiverged = !!$base?.diverged;
	$: baseBranchAheadOnly = baseBranchDiverged && !!$base?.divergedBehind?.length === false;
	$: divergenceTooltip = baseBranchAheadOnly
		? 'Your local target branch is ahead of its upstream'
		: 'Your local target branch has diverged from its upstream';
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
				<span class="text-9 text-bold">{$base?.behind || 0}</span>
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
						<Badge label={$base?.behind || 0} />
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
						style="width:0.75rem; height: 0.75rem"
						viewBox="0 0 14 14"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
					>
						<path
							fill-rule="evenodd"
							clip-rule="evenodd"
							d="M6.98091 0.599976C3.45242 0.599976 0.599976 3.47344 0.599976 7.02832C0.599976 9.86992 2.42763 12.2753 4.96308 13.1266C5.28007 13.1906 5.39619 12.9883 5.39619 12.8181C5.39619 12.6691 5.38574 12.1582 5.38574 11.626C3.61072 12.0092 3.24109 10.8597 3.24109 10.8597C2.95583 10.1147 2.53317 9.92321 2.53317 9.92321C1.9522 9.52941 2.57549 9.52941 2.57549 9.52941C3.21993 9.57199 3.55808 10.1893 3.55808 10.1893C4.12847 11.1683 5.04758 10.8917 5.41735 10.7214C5.47011 10.3063 5.63926 10.0189 5.81885 9.85934C4.40314 9.71031 2.91364 9.15691 2.91364 6.68768C2.91364 5.98525 3.16703 5.41055 3.56853 4.9636C3.50518 4.80399 3.28327 4.14401 3.63201 3.26068C3.63201 3.26068 4.17078 3.09036 5.38561 3.92053C5.90572 3.77982 6.4421 3.70824 6.98091 3.70763C7.51968 3.70763 8.06891 3.78221 8.57607 3.92053C9.79103 3.09036 10.3298 3.26068 10.3298 3.26068C10.6785 4.14401 10.4565 4.80399 10.3932 4.9636C10.8052 5.41055 11.0482 5.98525 11.0482 6.68768C11.0482 9.15691 9.55867 9.6996 8.13238 9.85934C8.36487 10.0615 8.56549 10.4446 8.56549 11.0513C8.56549 11.9133 8.55504 12.6052 8.55504 12.818C8.55504 12.9883 8.67129 13.1906 8.98815 13.1267C11.5236 12.2751 13.3513 9.86992 13.3513 7.02832C13.3617 3.47344 10.4988 0.599976 6.98091 0.599976Z"
							fill="currentColor"
						/>
					</svg>
				{:else}
					<Icon name="branch" />
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
