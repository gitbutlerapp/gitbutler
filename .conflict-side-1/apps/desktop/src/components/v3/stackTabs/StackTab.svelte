<script lang="ts">
	import StackTabMenu from '$components/v3/stackTabs/StackTabMenu.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { goto } from '$app/navigation';

	type Props = {
		name: string;
		projectId: string;
		stackId: string;
		anchors?: string[];

		selected?: boolean;
		href?: string;
		onNextTab?: () => void;
		onPrevTab?: () => void;
	};

	const { name, projectId, stackId, anchors, selected, href, onNextTab, onPrevTab }: Props =
		$props();

	let isMenuOpen = $state(false);

	function handleArrowNavigation(event: KeyboardEvent) {
		if (event.key === 'ArrowRight' || event.key === 'ArrowLeft') {
			event.preventDefault();
			const target = event.currentTarget as HTMLAnchorElement;
			const nextTab = target.nextElementSibling as HTMLAnchorElement;
			const prevTab = target.previousElementSibling as HTMLAnchorElement;

			if (event.key === 'ArrowRight') {
				onNextTab?.();
				if (nextTab) {
					nextTab.focus();
				}
			} else if (event.key === 'ArrowLeft') {
				onPrevTab?.();
				if (prevTab) {
					prevTab.focus();
				}
			}
		}
	}
</script>

<button
	type="button"
	class="tab"
	class:selected
	class:menu-open={isMenuOpen}
	tabindex="0"
	onkeydown={handleArrowNavigation}
	data-sveltekit-preload-data="hover"
	onclick={(e) => {
		e.preventDefault();
		e.stopPropagation();
		if (href) {
			goto(href, { replaceState: true, keepFocus: true });
		}
	}}
>
	{#if anchors}
		<div class="tab-icon">
			<Icon name={anchors.length > 0 ? 'chain-link' : 'branch-small'} verticalAlign="top" />
		</div>
	{/if}

	<div class="text-12 text-semibold tab-name">
		{name}
	</div>

	<div class="tab-menu-placeholder">
		<div class="truncation-gradient"></div>
	</div>

	<div class="menu-wrapper">
		<div class="truncation-gradient"></div>
		<StackTabMenu {projectId} {stackId} bind:isOpen={isMenuOpen} />
	</div>
</button>

<style lang="postcss">
	.tab {
		--menu-btn-size: 20px;
		--tab-menu-opacity: 0;
		--tab-menu-padding-right: 10px;
		--truncation-gradient-width: 14px;
		--truncation-gradient-stop: 70%;
		--truncation-gradient-color: var(--clr-stack-tab-inactive);
		--tab-background-color: var(--clr-stack-tab-inactive);

		position: relative;
		display: flex;
		align-items: center;
		padding: 0 0 0 12px;
		height: 44px;
		background: var(--tab-background-color);
		border-right: 1px solid var(--clr-border-2);
		overflow: hidden;
		min-width: 60px;
		scroll-snap-align: start;
		transition: transform var(--transition-medium);

		&::after {
			content: '';
			position: absolute;
			top: 0;
			left: 0;
			width: 100%;
			height: 2px;
			transform: translateY(-100%);
			transition: transform var(--transition-medium);
		}

		/* MODIFIERS */
		&:first-child {
			border-radius: var(--radius-ml) 0 0 0;
		}
		&:last-child {
			border-right: none;
		}
	}

	.selected {
		--tab-menu-opacity: 1;
		--truncation-gradient-color: var(--clr-stack-tab-active);
		--tab-background-color: var(--clr-stack-tab-active);

		&::after {
			transform: translateY(0);
			background: var(--clr-text-3);
			z-index: var(--z-ground);
		}

		.tab-name {
			margin-right: calc(
				var(--menu-btn-size) + var(--truncation-gradient-width) + var(--tab-menu-padding-right) -
					4px
			);
		}
	}

	.tab-icon {
		color: var(--clr-text-2);
		display: flex;
		align-items: center;
		box-shadow: inset 0 0 0 1px var(--clr-border-2);
		border-radius: var(--radius-s);
		width: 16px;
		height: 16px;
		line-height: 16px;
		margin-right: 8px;
	}

	.tab-name {
		position: relative;
		width: 100%;
		white-space: nowrap;
		margin-right: calc(var(--truncation-gradient-width));

		/* overflow: hidden; */
	}

	/* MENU AND TRUNCATION */

	.truncation-gradient {
		width: var(--truncation-gradient-width);
		transform-origin: right;
		height: 100%;
		background: linear-gradient(
			to right,
			oklch(from var(--truncation-gradient-color) l c h / 0) 0%,
			var(--truncation-gradient-color) var(--truncation-gradient-stop)
		);
	}

	.menu-wrapper {
		position: absolute;
		top: 0;
		right: 0;
		padding-right: var(--tab-menu-padding-right);
		display: flex;
		justify-content: flex-end;
		align-items: center;
		height: 100%;
		opacity: var(--tab-menu-opacity);
		background-color: var(--truncation-gradient-color);

		& .truncation-gradient {
			position: absolute;
			top: 0;
			left: 0;
			transform: translateX(-100%);
			/* background-color: red; */
		}
	}

	.tab-menu-placeholder {
		pointer-events: none;
		position: absolute;
		top: 0;
		right: 0;
		display: flex;
		justify-content: flex-end;
		width: var(--truncation-gradient-width);
		height: 100%;
		/* background-color: rgba(0, 255, 0, 0.2); */

		& .truncation-gradient {
			position: absolute;
			top: 0;
			left: 0;
			transform: scaleX(1.5);
			--truncation-gradient-stop: 60%;
			/* background-color: blue; */
		}
	}

	/* HOVERS AND STATES */
	.tab.menu-open,
	.tab:not(.selected):focus-within,
	.tab:not(.selected):hover {
		outline: none;
		--tab-menu-opacity: 1;
		--truncation-gradient-color: var(--clr-stack-tab-inactive-hover);
		--tab-background-color: var(--clr-stack-tab-inactive-hover);
	}

	.tab:focus {
		outline: none;
		&::after {
			background: var(--clr-theme-pop-element);
			transform: translateY(0);
		}
	}

	.tab:not(.selected):focus {
		--truncation-gradient-color: var(--clr-stack-tab-inactive-hover);
		--tab-background-color: var(--clr-stack-tab-inactive-hover);
	}

	.tab.selected:focus {
		--truncation-gradient-color: var(--clr-stack-tab-active);
		--tab-background-color: var(--clr-stack-tab-active);
	}
</style>
