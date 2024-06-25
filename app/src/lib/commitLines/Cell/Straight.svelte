<script lang="ts">
	import type { Color, Style } from '$lib/commitLines/types';

	interface Props {
		color: Color;
		style?: Style;
	}

	const { color, style }: Props = $props();
</script>

<div
	class="commit-line straight"
	class:none={color === 'none'}
	class:remote={color === 'remote'}
	class:local={color === 'local'}
	class:local-and-remote={color === 'localAndRemote'}
	class:shadow={color === 'shadow'}
	class:integrated={color === 'integrated'}
	class:dashed={style === 'dashed'}
></div>

<style lang="postcss">
	.straight {
		border-right: 2px var(--border-style) var(--border-color);
	}
</style>
