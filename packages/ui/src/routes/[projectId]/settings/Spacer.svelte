<script lang="ts">
	export let type: 'card' | 'surface' = 'surface';
</script>

<div
	class="h-[0.0625rem] shrink-0"
	style:background-color={type == 'surface'
		? 'var(--bg-surface-highlight)}'
		: 'var(--bg-card-highlight'}
/>
