<script lang="ts">
	export let color: 'info' | 'info-dark' | 'accent' | 'success' | 'error' | 'warning';
	export let clickable = false;
</script>

<div
	class="info-message text-base-11 text-semibold"
	class:color
	on:click
	role={clickable ? 'button' : undefined}
>
	<span class="label">
		<slot />
	</span>
</div>

<style lang="postcss">
	.info-message {
		display: flex;
		flex-direction: column;
	}
</style>
