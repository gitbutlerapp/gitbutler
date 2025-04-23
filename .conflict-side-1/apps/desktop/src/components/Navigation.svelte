<script lang="ts">
	import Branches from '$components/Branches.svelte';
	import EditButton from '$components/EditButton.svelte';
	import Footer from '$components/Footer.svelte';
	import ProjectSelector from '$components/ProjectSelector.svelte';
	import Resizer from '$components/Resizer.svelte';
	import TargetCard from '$components/TargetCard.svelte';
	import WorkspaceButton from '$components/WorkspaceButton.svelte';
	import { ModeService } from '$lib/mode/modeService';
	import { platformName } from '$lib/platform/platform';
	import { ShortcutService } from '$lib/shortcuts/shortcutService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import { env } from '$env/dynamic/public';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const minResizerWidth = 14;
	const minResizerRatio = 7;
	const width = persisted<number | undefined>(25, 'defaultTrayWidth_' + projectId);

	let viewport = $state<HTMLDivElement>();
	let isResizerHovered = $state(false);
	let isResizerDragging = $state(false);

	const isNavCollapsed = persisted<boolean>(false, 'projectNavCollapsed_' + projectId);

	const shortcutService = getContext(ShortcutService);
	const modeService = getContext(ModeService);
	const mode = $derived(modeService.mode);

	shortcutService.on('toggle-sidebar', () => {
		toggleNavCollapse();
	});

	function toggleNavCollapse() {
		$isNavCollapsed = !$isNavCollapsed;
	}
</script>

<aside class="navigation-wrapper">
	<div
		class="resizer-wrapper"
		tabindex="0"
		role="button"
		class:folding-button_folded={$isNavCollapsed}
	>
		{#if viewport}
			<Resizer
				{viewport}
				direction="right"
				minWidth={minResizerWidth}
				zIndex="var(--z-floating)"
				onDblClick={toggleNavCollapse}
				onWidth={(value) => ($width = value)}
				imitateBorder
				onHover={(isHovering) => {
					isResizerHovered = isHovering;
				}}
				onResizing={(isDragging) => {
					isResizerDragging = isDragging;
				}}
				onOverflow={(overflowValue) => {
					if (!$isNavCollapsed && overflowValue > minResizerRatio) {
						$isNavCollapsed = true;
					}

					if ($isNavCollapsed && overflowValue < minResizerRatio) {
						$isNavCollapsed = false;
					}
				}}
			/>
		{/if}

		<button
			type="button"
			aria-label="Collapse Navigation"
			class="folding-button"
			class:resizer-hovered={isResizerHovered || isResizerDragging}
			onmousedown={toggleNavCollapse}
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
		style:width={$width && !$isNavCollapsed ? $width + 'rem' : null}
		bind:this={viewport}
		role="menu"
	>
		<!-- condition prevents split second UI shift -->
		{#if platformName || env.PUBLIC_TESTING}
			<div class="navigation-top">
				{#if platformName === 'macos'}
					<div class="traffic-lights-placeholder" data-tauri-drag-region></div>
				{/if}
				<ProjectSelector isNavCollapsed={$isNavCollapsed} />
				<div class="domains">
					<TargetCard {projectId} isNavCollapsed={$isNavCollapsed} />
					{#if $mode?.type === 'OpenWorkspace'}
						<WorkspaceButton href={`/${projectId}/board`} isNavCollapsed={$isNavCollapsed} />
					{:else if $mode?.type === 'Edit'}
						<EditButton href={`/${projectId}/edit`} isNavCollapsed={$isNavCollapsed} />
					{/if}
				</div>
			</div>

			{#if !$isNavCollapsed}
				<Branches {projectId} />
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
				z-index: var(--z-floating);
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

	.navigation-top {
		display: flex;
		flex-direction: column;
		padding: 0 14px 14px 14px;
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
		z-index: var(--z-ground);
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

	.traffic-lights-placeholder {
		height: 30px;
	}

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
