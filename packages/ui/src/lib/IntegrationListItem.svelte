<script lang="ts" module>
	export interface Props {
		type: 'clear' | 'conflicted' | 'integrated';
	}
</script>

<script lang="ts">
	import Icon from '$lib/Icon.svelte';
</script>

<div class="integration-item">
	<div class="icon">
		<Icon name="remote-branch-small" />
	</div>
</div>

<style lang="postcss">
	.integration-item {
		display: flex;
		align-items: center;
		padding: 16px;
		border-radius: 8px;
		background-color: var(--color-background);
		box-shadow: 0px 4px 8px rgba(0, 0, 0, 0.1);
	}
</style>
