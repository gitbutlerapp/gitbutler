<script lang="ts">
	let { initialIndex }: { initialIndex?: number } = $props();
</script>

<div data-testid="floating-diff" data-initial-index={initialIndex}></div>
