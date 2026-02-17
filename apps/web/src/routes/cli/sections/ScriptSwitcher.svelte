<script lang="ts">
	interface Props {
		scriptsData: Record<string, any>;
		onScriptChange?: (scriptId: string) => void;
		selectedScript?: string;
		scriptProgress?: number;
	}

	const {
		scriptsData,
		onScriptChange,
		selectedScript = "stacked-branches",
		scriptProgress = 0,
	}: Props = $props();

	const scripts = Object.values(scriptsData);

	function handleScriptSelect(scriptId: string) {
		onScriptChange?.(scriptId);
	}
</script>

<div class="script-switcher">
	{#each scripts as script}
		<button
			type="button"
			class="script-button"
			class:active={selectedScript === script.id}
			onclick={() => handleScriptSelect(script.id)}
		>
			{#if selectedScript === script.id}
				<div
					class="script-switcher__scale-progress"
					style="width: calc({scriptProgress} * (100% - 40px))"
				></div>
			{/if}

			{#if script.icon}
				<div class="script-button__icon">
					{@html script.icon}
				</div>
			{/if}
			<span class="text-14 text-bold script-button__title">{script.title}</span>
		</button>
	{/each}
</div>

<style lang="postcss">
	.script-switcher {
		display: flex;
		padding: 4px;

		&::-webkit-scrollbar {
			display: none;
		}
	}

	.script-button {
		display: flex;
		z-index: 1;
		position: relative;
		flex-direction: column;
		align-items: flex-start;
		margin-left: -1px;
		padding: 16px;
		overflow: hidden;
		gap: 8px;
		border: 1px solid var(--clr-border-2);
		background-image: radial-gradient(
			circle,
			color-mix(in srgb, var(--clr-text-3) 24%, transparent) 1px,
			transparent 1px
		);
		background-size: 8px 8px;
		background-color: var(--clr-bg-2);
		transition:
			border-radius 0.1s ease,
			background 0.1s ease,
			transform 0.1s ease;

		&.active {
			z-index: 2;
			transform: translateY(-4px) scale(1.05);
			border-radius: var(--radius-xl);
			background: var(--clr-bg-1);
			pointer-events: none;
		}

		&:first-child {
			border-top-left-radius: var(--radius-xl);
			border-bottom-left-radius: var(--radius-xl);
		}

		&:last-child {
			border-top-right-radius: var(--radius-xl);
			border-bottom-right-radius: var(--radius-xl);
		}

		/* Round right corners when followed by active element */
		&:has(+ .active) {
			border-top-right-radius: var(--radius-xl);
			border-bottom-right-radius: var(--radius-xl);
		}

		/* Round left corners when preceded by active element */
		&.active + & {
			border-top-left-radius: var(--radius-xl);
			border-bottom-left-radius: var(--radius-xl);
		}

		&:hover {
			background-color: color-mix(in srgb, var(--clr-bg-1) 90%, var(--clr-bg-2));
		}
	}

	.script-switcher__scale-progress {
		position: absolute;
		top: 0;
		left: 20px;
		height: 4px;
		overflow: hidden;
		border-radius: 8px;
		background: var(--clr-theme-pop-element);
		transition: width 0.05s linear;
	}

	.script-button__icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		color: var(--clr-text-3);
		transition: color 0.15s ease;

		:global(svg) {
			width: 100%;
			height: 100%;
		}
	}

	@media (max-width: 700px) {
		.script-switcher {
			padding: 6px 16px 0;
			overflow-x: auto;
			scroll-padding: 16px;
			/* snap to children */
			scroll-snap-type: x mandatory;
		}

		.script-button {
			min-width: fit-content;
			/* snap to center */
			scroll-snap-align: start;
		}
	}
</style>
