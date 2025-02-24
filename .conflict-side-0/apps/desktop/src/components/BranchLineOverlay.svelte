<script lang="ts" module>
	// Global state to track the currently hovered overlay
	let currentHoveredId: symbol | null = $state(null);
</script>

<script lang="ts">
	interface Props {
		hovered: boolean;
	}

	const { hovered }: Props = $props();

	// Unique ID for this overlay instance
	const overlayId = Symbol('overlay');

	// Update global state when this overlay is hovered
	$effect(() => {
		if (hovered) {
			currentHoveredId = overlayId;
		} else {
			// Always clear if this was the hovered one, or if no hover state
			if (currentHoveredId === overlayId) {
				currentHoveredId = null;
			}
		}
	});

	// Ensure cleanup on unmount
	$effect(() => {
		return () => {
			if (currentHoveredId === overlayId) {
				currentHoveredId = null;
			}
		};
	});

	// Check if other overlays are dimmed (another overlay is hovered, not this one)
	const isDimmed = $derived(currentHoveredId !== null && currentHoveredId !== overlayId);
</script>

<div class="container" class:hovered class:dimmed={isDimmed}>
	<div class="indicator"></div>
</div>

<style lang="postcss">
	.container {
		z-index: var(--z-floating);
		position: relative;
		top: var(--y-offset);
		align-items: center;
		width: 100%;
		pointer-events: none;

		&.hovered {
			& .indicator {
				width: calc(100% - 70px);
				width: 100%;
			}
		}

		&.dimmed {
			& .indicator {
				opacity: 0.4;
			}
		}
	}

	.indicator {
		position: absolute;
		top: 50%;
		left: 50%;
		width: 60%;
		height: 3px;
		transform: translate(-50%, -50%);
		border-radius: var(--radius-m);
		background-color: transparent;
		background-color: var(--clr-theme-pop-element);

		transition:
			width 0.08s ease,
			opacity 0.2s ease;
	}
</style>
