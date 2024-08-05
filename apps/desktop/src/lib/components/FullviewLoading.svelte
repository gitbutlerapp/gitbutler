<script lang="ts">
	import Icon from '@gitbutler/ui/icon/Icon.svelte';
</script>

<div class="loading" data-tauri-drag-region><Icon name="spinner" /></div>

<style>
	.loading {
		display: flex;
		justify-content: center;
		align-items: center;
		height: 100%;
		width: 100%;
	}
</style>
