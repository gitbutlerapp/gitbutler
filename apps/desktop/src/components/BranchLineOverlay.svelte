<script lang="ts">
	interface Props {
		hovered: boolean;
		activated: boolean;
	}

	const { hovered, activated }: Props = $props();
</script>

<div data-testid="BranchListInsertionDropzone" class="container" class:activated class:hovered>
	<div class="indicator"></div>
</div>

<style lang="postcss">
	.container {
		display: none;
		z-index: var(--z-floating);
		position: relative;
		top: var(--y-offset);
		align-items: center;
		width: 100%;

		/* It is very important that all children are pointer-events: none */
		/* https://stackoverflow.com/questions/7110353/html5-dragleave-fired-when-hovering-a-child-element */
		pointer-events: none;
		&.activated {
			display: flex;
		}

		&.hovered {
			& .indicator {
				width: 65%;
				/* height: 4px; */
				/* transform: translate(-50%, -50%); */
				/* background-color: var(--clr-scale-pop-60); */
				/* box-shadow: 0 0 8px var(--clr-scale-pop-70); */
			}
		}
	}

	.indicator {
		position: absolute;
		top: 50%;
		left: 50%;
		width: 45%;
		height: 3px;
		transform: translate(-50%, -50%);
		border-radius: var(--radius-m);
		background-color: transparent;
		background-color: var(--clr-theme-pop-element);

		transition: width 0.1s ease;
	}
</style>
