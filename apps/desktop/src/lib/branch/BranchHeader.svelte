<script lang="ts">
	import ActiveBranchStatus from './ActiveBranchStatus.svelte';
	import BranchLabel from './BranchLabel.svelte';
	import BranchLaneContextMenu from './BranchLaneContextMenu.svelte';
	import DefaultTargetButton from './DefaultTargetButton.svelte';
	import PullRequestButton from '../pr/PullRequestButton.svelte';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import { stackingFeature } from '$lib/config/uiFeatureFlags';
	import { mapErrorToToast } from '$lib/gitHost/github/errorMap';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import { getGitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
	import { getGitHostPrMonitor } from '$lib/gitHost/interface/gitHostPrMonitor';
	import { getGitHostPrService } from '$lib/gitHost/interface/gitHostPrService';
	import { showError, showToast } from '$lib/notifications/toasts';
	import PrDetailsModal, { type CreatePrParams } from '$lib/pr/PrDetailsModal.svelte';
	import { getBranchNameFromRef } from '$lib/utils/branch';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { sleep } from '$lib/utils/sleep';
	import { error } from '$lib/utils/toasts';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranch } from '$lib/vbranches/types';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { tick } from 'svelte';
	import type { PullRequest } from '$lib/gitHost/interface/types';
	import type { Persisted } from '$lib/persisted/persisted';

	interface Props {
		uncommittedChanges?: number;
		isLaneCollapsed: Persisted<boolean>;
		onGenerateBranchName: () => void;
	}

	const { uncommittedChanges = 0, isLaneCollapsed, onGenerateBranchName }: Props = $props();

	const branchController = getContext(BranchController);
	const baseBranchService = getContext(BaseBranchService);
	const baseBranch = getContextStore(BaseBranch);
	const prService = getGitHostPrService();
	const gitListService = getGitHostListingService();
	const branchStore = getContextStore(VirtualBranch);
	const prMonitor = getGitHostPrMonitor();
	const gitHost = getGitHost();

	const baseBranchName = $derived($baseBranch.shortName);
	const branch = $derived($branchStore);
	const pr = $derived($prMonitor?.pr);

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let useDraftPr = $state<boolean>(false);
	let prDetailsModal = $state<ReturnType<typeof PrDetailsModal>>();
	let meatballButtonEl = $state<HTMLDivElement>();
	let isLoading = $state(false);
	let isTargetBranchAnimated = $state(false);

	function handleBranchNameChange(title: string) {
		if (title === '') return;

		branchController.updateBranchName(branch.id, title);
	}

	function expandLane() {
		$isLaneCollapsed = false;
	}

	function collapseLane() {
		$isLaneCollapsed = true;
	}

	const hasIntegratedCommits = $derived(branch.commits?.some((b) => b.isIntegrated));

	let headerInfoHeight = $state(0);

	async function createPr(params: CreatePrParams): Promise<PullRequest | undefined> {
		if (!$gitHost) {
			error('Pull request service not available');
			return;
		}

		isLoading = true;
		try {
			let upstreamBranchName = branch.upstreamName;

			if (branch.commits.some((c) => !c.isRemote)) {
				const firstPush = !branch.upstream;
				const { refname, remote } = await branchController.pushBranch(
					branch.id,
					branch.requiresForce
				);
				upstreamBranchName = getBranchNameFromRef(refname, remote);

				if (firstPush) {
					// TODO: fix this hack for reactively available prService.
					await sleep(500);
				}
			}

			if (!baseBranchName) {
				error('No base branch name determined');
				return;
			}

			if (!upstreamBranchName) {
				error('No upstream branch name determined');
				return;
			}

			if (!$prService) {
				error('Pull request service not available');
				return;
			}

			await $prService.createPr({
				title: params.title,
				body: params.body,
				draft: params.draft,
				baseBranchName,
				upstreamName: upstreamBranchName
			});
		} catch (err: any) {
			console.error(err);
			const toast = mapErrorToToast(err);
			if (toast) showToast(toast);
			else showError('Error while creating pull request', err);
		} finally {
			isLoading = false;
		}
		await $gitListService?.refresh();
		baseBranchService.fetchFromRemotes();
	}

	async function handleCreatePR(draft: boolean) {
		useDraftPr = draft;
		await tick();
		prDetailsModal?.show();
	}
</script>

{#if $isLaneCollapsed}
	<div
		class="card collapsed-lane"
		class:collapsed-lane_target-branch={branch.selectedForChanges}
		onkeydown={(e) => e.key === 'Enter' && expandLane()}
		tabindex="0"
		role="button"
	>
		<div class="collapsed-lane__actions">
			<div class="draggable" data-drag-handle>
				<Icon name="draggable" />
			</div>
			<Button style="ghost" outline icon="unfold-lane" tooltip="Expand lane" onclick={expandLane} />
		</div>

		<div class="collapsed-lane__info-wrap" bind:clientHeight={headerInfoHeight}>
			<div class="collapsed-lane__info" style="width: {headerInfoHeight}px">
				<div class="collapsed-lane__label-wrap">
					<h3 class="collapsed-lane__label text-13 text-bold">
						{branch.name}
					</h3>
					{#if uncommittedChanges > 0}
						<Button
							size="tag"
							clickable={false}
							style="warning"
							kind="soft"
							tooltip="Uncommitted changes"
						>
							{uncommittedChanges}
							{uncommittedChanges === 1 ? 'change' : 'changes'}
						</Button>
					{/if}
				</div>

				<div class="collapsed-lane__info__details">
					<ActiveBranchStatus
						{hasIntegratedCommits}
						remoteExists={!!branch.upstream}
						isLaneCollapsed={$isLaneCollapsed}
					/>
					{#if branch.selectedForChanges}
						<Button style="pop" kind="soft" size="tag" clickable={false} icon="target"
							>Default branch</Button
						>
					{/if}
				</div>
			</div>
		</div>
	</div>
{:else}
	<div class="header__wrapper" class:header__wrapper--stacking={$stackingFeature}>
		<div
			class="header card"
			class:header_card--stacking={$stackingFeature}
			class:header_target-branch={branch.selectedForChanges}
			class:header_target-branch-animation={isTargetBranchAnimated && branch.selectedForChanges}
		>
			<div class="header__info-wrapper">
				<div class="draggable" data-drag-handle>
					<Icon name="draggable" />
				</div>

				<div class:header__info={!$stackingFeature} class:stacking-header__info={$stackingFeature}>
					<BranchLabel name={branch.name} onChange={(name) => handleBranchNameChange(name)} />
					{#if $stackingFeature}
						<span class="button-group">
							<DefaultTargetButton
								selectedForChanges={branch.selectedForChanges}
								onclick={async () => {
									isTargetBranchAnimated = true;
									await branchController.setSelectedForChanges(branch.id);
								}}
							/>
							<Button
								bind:el={meatballButtonEl}
								style="ghost"
								icon="kebab"
								onclick={() => {
									contextMenu?.toggle();
								}}
							/>
							<BranchLaneContextMenu
								bind:contextMenuEl={contextMenu}
								target={meatballButtonEl}
								onCollapse={collapseLane}
								{onGenerateBranchName}
							/>
						</span>
					{:else}
						<div class="header__remote-branch">
							<ActiveBranchStatus
								{hasIntegratedCommits}
								remoteExists={!!branch.upstream}
								isLaneCollapsed={$isLaneCollapsed}
							/>

							{#await branch.isMergeable then isMergeable}
								{#if !isMergeable}
									<Button
										size="tag"
										clickable={false}
										icon="locked-small"
										style="warning"
										tooltip="Applying this branch will add merge conflict markers that you will have to resolve"
									>
										Conflict
									</Button>
								{/if}
							{/await}
						</div>
					{/if}
				</div>
			</div>

			{#if !$stackingFeature}
				<div class="header__actions">
					<div class="header__buttons">
						<DefaultTargetButton
							selectedForChanges={branch.selectedForChanges}
							onclick={async () => {
								isTargetBranchAnimated = true;
								await branchController.setSelectedForChanges(branch.id);
							}}
						/>
					</div>

					<div class="relative">
						<div class="header__buttons">
							{#if !$pr}
								<PullRequestButton
									click={async ({ draft }) => await handleCreatePR(draft)}
									disabled={branch.commits.length === 0 || !$gitHost || !$prService}
									tooltip={!$gitHost || !$prService
										? 'You can enable git host integration in the settings'
										: ''}
									loading={isLoading}
								/>
							{/if}
							<Button
								bind:el={meatballButtonEl}
								style="ghost"
								outline
								icon="kebab"
								onclick={() => {
									contextMenu?.toggle();
								}}
							/>
							<BranchLaneContextMenu
								bind:contextMenuEl={contextMenu}
								target={meatballButtonEl}
								onCollapse={collapseLane}
								{onGenerateBranchName}
							/>
						</div>
					</div>
				</div>
			{/if}
		</div>
		<div class="header__top-overlay" data-remove-from-draggable data-tauri-drag-region></div>
	</div>
{/if}

<PrDetailsModal
	bind:this={prDetailsModal}
	type="preview"
	onCreatePr={createPr}
	draft={useDraftPr}
/>

<style>
	.header__wrapper {
		z-index: var(--z-lifted);
		position: sticky;
		top: 12px;
		padding-bottom: 8px;
	}
	.header__wrapper--stacking {
		padding-bottom: unset !important;

		& .header__info-wrapper .draggable {
			height: auto;
		}
	}
	.header_card--stacking {
		border-bottom-right-radius: 0px;
		border-bottom-left-radius: 0px;
		border-bottom-width: 0px;
	}
	.header {
		z-index: var(--z-lifted);
		position: relative;
		flex-direction: column;
		gap: 2px;
		transition:
			border-color 0.12s ease-in-out,
			box-shadow 0.12s ease-in-out;
	}
	.header_target-branch {
		border-color: var(--clr-theme-pop-element);
		box-shadow: 0 4px 0 var(--clr-theme-pop-element);
		margin-bottom: 4px;
	}
	.header_target-branch-animation {
		animation: setTargetAnimation 0.3s ease-in-out forwards;
	}
	@keyframes setTargetAnimation {
		0% {
		}
		40% {
			transform: scale(1.017) rotate(1deg);
		}
		50% {
			border-color: var(--clr-theme-pop-element);
			box-shadow: 0 4px 0 var(--clr-theme-pop-element);
			margin-bottom: 4px;
		}
		70%,
		100% {
			transform: scale(1);
			border-color: var(--clr-theme-pop-element);
			box-shadow: 0 4px 0 var(--clr-theme-pop-element);
			margin-bottom: 4px;
		}
	}

	.header__top-overlay {
		z-index: var(--z-ground);
		position: absolute;
		top: -16px;
		left: 0;
		width: 100%;
		height: 20px;
		background: var(--clr-bg-2);
	}
	.header__info-wrapper {
		display: flex;
		gap: 2px;
		padding: 10px;
	}
	.header__info {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		gap: 10px;
	}
	/* TODO: Remove me after stacking feature toggle has been removed. */
	.stacking-header__info {
		flex: 1;
		display: flex;
		overflow: hidden;
		justify-content: space-between;
		align-items: center;
		gap: 10px;
	}
	.button-group {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.header__actions {
		display: flex;
		gap: 4px;
		background: var(--clr-bg-1);
		border-top: 1px solid var(--clr-border-2);
		padding: 14px;
		justify-content: space-between;
		border-radius: 0 0 var(--radius-m) var(--radius-m);
		user-select: none;
	}

	.header__buttons {
		display: flex;
		position: relative;
		gap: 4px;
	}
	.draggable {
		display: flex;
		height: fit-content;
		align-items: center;
		cursor: grab;
		padding: 2px 2px 0 0;
		color: var(--clr-scale-ntrl-50);
		transition: color var(--transition-slow);

		&:hover {
			color: var(--clr-scale-ntrl-40);
		}
	}

	.header__remote-branch {
		color: var(--clr-scale-ntrl-50);
		padding-left: 2px;
		padding-right: 2px;
		display: flex;
		gap: 4px;
		text-overflow: ellipsis;
		overflow-x: hidden;
		white-space: nowrap;
		align-items: center;
	}

	/*  COLLAPSIBLE LANE */

	.collapsed-lane {
		cursor: default;
		user-select: none;
		align-items: center;
		height: 100%;
		width: 48px;
		overflow: hidden;
		gap: 8px;
		padding: 8px 8px 20px;

		&:focus-within {
			outline: none;
		}
	}

	.collapsed-lane_target-branch {
		border-color: var(--clr-theme-pop-element);
	}

	.collapsed-lane__actions {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 10px;
	}

	/*  */

	.collapsed-lane__info-wrap {
		display: flex;
		height: 100%;
	}

	.collapsed-lane__info {
		display: flex;
		justify-content: space-between;
		gap: 8px;
		transform: rotate(-90deg);
		direction: ltr;
	}

	/*  */

	.collapsed-lane__info__details {
		display: flex;
		flex-direction: row-reverse;
		align-items: center;
		gap: 4px;
	}

	.collapsed-lane__label-wrap {
		overflow: hidden;
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.collapsed-lane__label {
		color: var(--clr-scale-ntrl-0);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
