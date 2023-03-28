<script lang="ts">
    export let leftLabel: string;
    export let leftAction: function;
    export let wide = false;
</script>
{#if $$slots.middle}
	<div>
		<slot name="left" />
		<slot name="middle" />
		<slot name="right" />
	</div>
{:else}
	<div>
		<slot name="left" />
		<slot name="right" />
	</div>
{/if}
