<script lang="ts">
	import { goto } from "$app/navigation";
	import { onMount, type Snippet } from "svelte";

	interface Props {
		currentPage?: "home" | "cli";
		descriptionContent: Snippet;
	}

	const { currentPage = "home", descriptionContent }: Props = $props();

	let clientButtonWidth = $state(0);
	let cliButtonWidth = $state(0);
	let clientButton: HTMLButtonElement;
	let cliButton: HTMLButtonElement;
	let hoveredOption = $state<"home" | "cli" | null>(null);
	let enableTransitions = $state(false);

	const TOGGLE_GAP = 4; // Gap between toggle buttons

	const hasValidMeasurements = $derived(clientButtonWidth > 0 && cliButtonWidth > 0);

	function updateButtonWidths() {
		if (clientButton && cliButton) {
			clientButtonWidth = clientButton.clientWidth;
			cliButtonWidth = cliButton.clientWidth;
		}
	}

	onMount(() => {
		updateButtonWidths();

		const resizeObserver = new ResizeObserver(updateButtonWidths);
		if (clientButton) resizeObserver.observe(clientButton);
		if (cliButton) resizeObserver.observe(cliButton);

		// Wait for fonts to load before enabling transitions
		document.fonts.ready.then(() => {
			// Update measurements after fonts are loaded
			updateButtonWidths();

			// Enable transitions only after fonts are loaded and measurements are final
			requestAnimationFrame(() => {
				enableTransitions = true;
			});
		});

		return () => resizeObserver.disconnect();
	});

	const activeOption = $derived(hoveredOption || currentPage);
</script>

<h1 class="title">
	Git, <i class="but-text"
		><span>But</span>
		<svg
			class="but-text__underline"
			width="134"
			height="59"
			viewBox="0 0 134 59"
			fill="none"
			xmlns="http://www.w3.org/2000/svg"
		>
			<path d="M10.307 38.7735C32.1083 27.5371 80.2447 17.3196 129.361 25.3953" stroke-width="45" />
		</svg>
	</i> Better
</h1>

<div class="description-wrapper">
	<div class="toggle-switch" role="group" aria-label="View mode selection">
		<button
			bind:this={clientButton}
			type="button"
			class="toggle-option"
			class:active={currentPage === "home"}
			class:hovered={hoveredOption === "home" && currentPage !== "home"}
			class:dimmed={hoveredOption !== null && hoveredOption !== "home" && currentPage === "home"}
			onclick={() => goto("/")}
			onmouseenter={() => (hoveredOption = "home")}
			onmouseleave={() => (hoveredOption = null)}
			aria-pressed={currentPage === "home"}
		>
			Desktop
		</button>
		<button
			bind:this={cliButton}
			type="button"
			class="toggle-option"
			class:active={currentPage === "cli"}
			class:hovered={hoveredOption === "cli" && currentPage !== "cli"}
			class:dimmed={hoveredOption !== null && hoveredOption !== "cli" && currentPage === "cli"}
			onclick={() => goto("/cli")}
			onmouseenter={() => (hoveredOption = "cli")}
			onmouseleave={() => (hoveredOption = null)}
			aria-pressed={currentPage === "cli"}
		>
			CLI
		</button>

		<div
			class="toggle-background"
			class:visible={hasValidMeasurements}
			class:enable-transitions={enableTransitions}
			class:dimmed={hoveredOption !== null && hoveredOption !== currentPage}
			style:width="{activeOption === 'cli' ? cliButtonWidth : clientButtonWidth}px"
			style:transform={activeOption === "cli"
				? `translateX(${clientButtonWidth + TOGGLE_GAP}px)`
				: "translateX(0)"}
			aria-hidden="true"
		></div>
	</div>

	<p class="description">
		{@render descriptionContent()}
	</p>
</div>

<style lang="postcss">
	.title {
		margin-bottom: 32px;
		font-size: 82px;
		line-height: 1;
		font-family: var(--font-accent);
	}

	.but-text {
		display: inline-flex;
		position: relative;

		& span {
			z-index: 1;
			position: relative;
		}
	}

	.but-text__underline {
		z-index: 0;
		position: absolute;
		bottom: -8px;
		left: -10%;
		width: 120%;
		height: auto;
		pointer-events: none;

		path {
			stroke: var(--clr-core-pop-70);
			opacity: 0.7;
		}
	}

	:global(.dark) .but-text__underline path {
		stroke: var(--clr-core-pop-30);
		opacity: 0.8;
	}

	.description-wrapper {
		display: flex;
		align-items: flex-start;
		margin-bottom: 20px;
		gap: 32px;
	}

	.description {
		max-width: 520px;
		padding-top: 4px;
		color: var(--clr-text-2);
		font-size: 16px;
		line-height: 1.5;
	}

	.toggle-switch {
		display: inline-flex;
		position: relative;
		padding: 8px;
		gap: 4px;
		border: 1px solid var(--clr-border-2);
		border-radius: 100px;
	}

	.toggle-background {
		z-index: 0;
		position: absolute;
		top: 8px;
		bottom: 8px;
		left: 8px;
		border-radius: 100px;
		background-color: var(--clr-theme-gray-element);
		opacity: 0;

		&.enable-transitions {
			transition:
				transform 0.4s cubic-bezier(0.34, 1.2, 0.6, 1),
				width 0.3s ease,
				opacity 0.15s ease,
				background-color 0.2s ease;
		}

		&.visible {
			opacity: 1;
		}

		&.dimmed {
			background-color: var(--clr-core-pop-70);
			opacity: 0.6;
		}
	}

	/* dark mode */
	:global(.dark) .toggle-background {
		&.dimmed {
			background-color: var(--clr-core-pop-40);
			opacity: 0.4;
		}
	}

	.toggle-option {
		display: flex;
		z-index: 1;
		padding: 12px 20px;
		font-size: 40px;
		line-height: 1;
		font-family: var(--font-accent);
		cursor: pointer;
		transition: color 0.2s ease;

		&.active {
			color: var(--clr-bg-1);
			pointer-events: none;
		}

		&.dimmed {
			color: var(--clr-text-2);
		}
	}

	@media (--tablet-viewport) {
		.description-wrapper {
			flex-direction: column;
			gap: 24px;
		}
		.toggle-switch {
			width: 100%;
		}

		.toggle-option {
			flex: 1;
			justify-content: center;
			font-size: 32px;
		}
	}

	@media (--mobile-viewport) {
		.title {
			margin-bottom: 16px;
			font-size: 58px;
		}

		.description-wrapper {
			gap: 16px;
		}
	}
</style>
