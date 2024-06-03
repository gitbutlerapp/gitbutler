<script lang="ts">
	/// Hover target overlap size in rem
	export let overlapSize: number = 0.5;
	/// Hover target overlap size in rem
	export let offset: number = 0;
	/// Padding size in rem
	export let padding: number = 0;
</script>

<div class="container">
	<div
		class="hoverTarget"
		style:--overlap-size={`${overlapSize}rem`}
		style:--offset={`${offset}rem`}
		style:--padding={`${padding}rem`}
	>
		<div class="menu">
			<slot></slot>
		</div>
	</div>
</div>

<style lang="postcss">
	.container {
		width: 100%;
	}

	.hoverTarget {
		height: calc(var(--overlap-size) * 2);

		margin-top: calc(var(--overlap-size) * -1 + var(--offset));
		margin-bottom: calc(var(--overlap-size) * -1 - var(--offset));

		padding-left: var(--padding);
		padding-right: var(--padding);

		position: relative;

		display: flex;
		align-items: center;

		z-index: var(--z-lifted);

		&:hover {
			& .menu {
				display: flex;
			}
		}
	}

	.menu {
		display: none;

		width: fit-content;

		height: 2rem;

		margin-left: auto;

		align-items: center;

		gap: var(--size-4);

		padding: var(--size-4);
		background-color: var(--clr-bg-1);
		border-radius: var(--radius-l);
		border: 1px var(--clr-border-3) solid;
	}
</style>
