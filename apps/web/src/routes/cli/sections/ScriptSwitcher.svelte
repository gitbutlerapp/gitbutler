<script lang="ts">
	interface Props {
		scriptsData: Record<string, any>;
		onScriptChange?: (scriptId: string) => void;
	}

	const { scriptsData, onScriptChange }: Props = $props();

	let selectedScript = $state('parallel-branches');

	const scripts = Object.values(scriptsData);

	function handleScriptSelect(scriptId: string) {
		selectedScript = scriptId;
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
		/* overflow: hidden; */

		&::-webkit-scrollbar {
			display: none;
		}
	}

	.script-button {
		display: flex;
		/* flex: 1; */
		flex-direction: column;
		align-items: flex-start;
		margin-left: -1px;
		padding: 16px;
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
			transform: translateY(-4px) scale(1.05);

			border-radius: var(--radius-xl);
			background: var(--clr-bg-1);
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
