<script lang="ts">
	import { SEMANTIC_ZONES } from '../constants/colorScales';
</script>

<div class="semantic-zones">
	{#each SEMANTIC_ZONES as zone, i}
		<div class="zone" style="--zone-index: {i}">
			<div class="text-14 text-bold zone-label">{zone.label}</div>
			<div class="text-12 zone-range">{zone.range}</div>
		</div>
	{/each}
</div>

<style>
	.semantic-zones {
		display: grid;
		grid-template-columns: subgrid;
		grid-column: 1 / -1;
		align-items: center;
		gap: 8px 0;
	}

	.zone {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	/* Background: 95-90 = columns 1-2 */
	.zone:nth-child(1) {
		grid-column: 1 / 3;
	}

	/* Soft: 80-60 = columns 3-5 */
	.zone:nth-child(2) {
		grid-column: 3 / 6;
	}

	/* Solid: 50-40 = columns 6-9 (50 spans 3, 40 spans 1) */
	.zone:nth-child(3) {
		grid-column: 6 / 10;
	}

	/* Text: 30-0 = columns 10-13 */
	.zone:nth-child(4) {
		grid-column: 10 / 14;
	}

	.zone-label {
		color: var(--clr-text-1);
		text-transform: uppercase;
	}

	.zone-range {
		color: var(--clr-text-2);
	}
</style>
