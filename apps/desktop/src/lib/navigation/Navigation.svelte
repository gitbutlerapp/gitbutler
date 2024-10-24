<script lang="ts">
	import Branches from './Branches.svelte';
	import Footer from './Footer.svelte';
	import ProjectSelector from './ProjectSelector.svelte';
	import TargetCard from './TargetCard.svelte';
	import WorkspaceButton from './WorkspaceButton.svelte';
	import Resizer from '../shared/Resizer.svelte';
	import { ProjectService } from '$lib/backend/projects';
	import { featureTopics } from '$lib/config/uiFeatureFlags';
	import { ModeService } from '$lib/modes/service';
	import CloudSeriesButton from '$lib/navigation/CloudSeriesButton.svelte';
	import EditButton from '$lib/navigation/EditButton.svelte';
	import TopicsButton from '$lib/navigation/TopicsButton.svelte';
	import { platformName } from '$lib/platform/platform';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { getContext, getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import { env } from '$env/dynamic/public';

	const minResizerWidth = 280;
	const minResizerRatio = 150;
	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	const projectService = getContext(ProjectService);
	const cloudEnabled = projectService.cloudEnabled;
	const projectId = projectService.projectId;
	const defaultTrayWidthRem = persisted<number | undefined>(
		undefined,
		'defaulTrayWidth_ ' + projectId
	);

	let viewport: HTMLDivElement;
	let isResizerHovered = false;
	let isResizerDragging = false;

	const isNavCollapsed = persisted<boolean>(false, 'projectNavCollapsed_' + projectId);

	function toggleNavCollapse() {
		$isNavCollapsed = !$isNavCollapsed;
	}

	const handleKeyDown = createKeybind({
		'$mod+/': () => {
			toggleNavCollapse();
		}
	});

	const modeService = getContext(ModeService);
	const mode = modeService.mode;

	const topicsEnabled = featureTopics();
</script>

<svelte:window on:keydown={handleKeyDown} />

<aside class="navigation-wrapper">
	<div
		class="resizer-wrapper"
		tabindex="0"
		role="button"
		class:folding-button_folded={$isNavCollapsed}
	>
		<Resizer
			{viewport}
			direction="right"
			minWidth={minResizerWidth}
			defaultLineColor="var(--clr-border-2)"
			zIndex="var(--z-floating)"
			on:dblclick={toggleNavCollapse}
			on:width={(e) => {
				$defaultTrayWidthRem = e.detail / (16 * $userSettings.zoom);
			}}
			on:hover={(e) => {
				isResizerHovered = e.detail;
			}}
			on:resizing={(e) => {
				isResizerDragging = e.detail;
			}}
			on:overflowValue={(e) => {
				const overflowValue = e.detail;

				if (!$isNavCollapsed && overflowValue > minResizerRatio) {
					$isNavCollapsed = true;
				}

				if ($isNavCollapsed && overflowValue < minResizerRatio) {
					$isNavCollapsed = false;
				}
			}}
		/>

		<button
			type="button"
			class="folding-button"
			class:resizer-hovered={isResizerHovered || isResizerDragging}
			on:mousedown={toggleNavCollapse}
		>
			<svg viewBox="0 0 6 11" fill="none" xmlns="http://www.w3.org/2000/svg">
				<path
					d="M5 1.25L1.59055 5.08564C1.25376 5.46452 1.25376 6.03548 1.59055 6.41436L5 10.25"
					stroke-width="1.5"
					vector-effect="non-scaling-stroke"
				/>
			</svg>
		</button>
	</div>

	<div
		class="navigation"
		class:collapsed={$isNavCollapsed}
		style:width={$defaultTrayWidthRem && !$isNavCollapsed ? $defaultTrayWidthRem + 'rem' : null}
		bind:this={viewport}
		role="menu"
	>
		<!-- condition prevents split second UI shift -->
		{#if $platformName || env.PUBLIC_TESTING}
			<div class="navigation-top">
				{#if $platformName === 'darwin'}
					<div class="drag-region" data-tauri-drag-region></div>
				{/if}
				<ProjectSelector isNavCollapsed={$isNavCollapsed} />
				<div class="domains">
					<TargetCard isNavCollapsed={$isNavCollapsed} />
					{#if $mode?.type === 'OpenWorkspace'}
						<WorkspaceButton href={`/${projectId}/board`} isNavCollapsed={$isNavCollapsed} />
					{:else if $mode?.type === 'Edit'}
						<EditButton href={`/${projectId}/edit`} isNavCollapsed={$isNavCollapsed} />
					{/if}

					{#if $topicsEnabled}
						<TopicsButton href={`/${projectId}/topics`} isNavCollapsed={$isNavCollapsed} />
					{/if}

					{#if $cloudEnabled}
						<CloudSeriesButton href={`/${projectId}/series`} isNavCollapsed={$isNavCollapsed} />
					{/if}
				</div>
			</div>

			{#if !$isNavCollapsed}
				<Branches />
			{/if}
			<Footer {projectId} isNavCollapsed={$isNavCollapsed} />
		{/if}
	</div>
</aside>

<style lang="postcss">
	.navigation-wrapper {
		display: flex;
		position: relative;

		&:hover:not(.hide-fold-button) {
			& .folding-button {
				pointer-events: auto;
				opacity: 1;
				right: -6px;
			}
		}
	}

	.navigation {
		width: 280px;
		display: flex;
		flex-direction: column;
		position: relative;
		background: var(--clr-bg-1);
		max-height: 100%;
		user-select: none;
	}

	.drag-region {
		flex-shrink: 0;
		height: 30px;
	}
	.navigation-top {
		display: flex;
		flex-direction: column;
		padding-bottom: 24px;
		padding-left: 14px;
		padding-right: 14px;
	}
	.domains {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.resizer-wrapper {
		position: absolute;
		top: 0;
		right: 0;
		height: 100%;
		width: 4px;
	}

	/* FOLDING BUTTON */

	.folding-button {
		z-index: var(--z-floating);
		display: flex;
		align-items: center;
		justify-content: center;
		position: absolute;
		right: -4px;
		top: 50%;
		width: 14px;
		height: 28px;
		background: var(--clr-bg-1);
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);
		pointer-events: none;
		opacity: 0;
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast),
			opacity var(--transition-medium),
			right var(--transition-fast);

		& svg {
			stroke: var(--clr-scale-ntrl-60);
			transition: stroke var(--transition-fast);
			width: 45%;
		}

		&:hover {
			& svg {
				stroke: var(--clr-scale-ntrl-50);
			}
		}
	}

	.folding-button_folded {
		& svg {
			transform: rotate(180deg);
		}
	}

	/* MODIFIERS */

	.navigation.collapsed {
		width: auto;
		justify-content: space-between;
		padding-bottom: 16px;
	}

	.resizer-hovered {
		background-color: var(--resizer-color);
		border: 1px solid var(--resizer-color);
		transition-delay: 0.1s;

		& svg {
			stroke: var(--clr-scale-ntrl-100);
			transition-delay: 0.1s;
		}
	}
</style>
