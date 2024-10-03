<script lang="ts">
	// Props for the component (with type annotations)
	export let diffArray: string[] = [];
	console.log(diffArray);
</script>

<div class="diff">
	<div class="gutter">
		{#each diffArray as line}
			<div class="line">{line.right}</div>
		{/each}
	</div>
	<div class="lines">
		{#each diffArray as line}
			<div class="line {line.type}">{line.line}</div>
		{/each}
	</div>
</div>

<style>
	.diff {
		display: flex;
		font-family: monospace;
		white-space: pre;
		background-color: #fff;
		padding: 8px;
		border: 1px solid #ccc;
		border-radius: 10px;
		width: 100%;
	}
	.gutter {
		padding: 0 8px;
	}
	.lines {
		width: 100%;
	}
	.line {
		padding: 2px;
	}
	.diff .added {
		background-color: #dfd;
	}
	.diff .removed {
		background-color: #fdd;
	}
</style>
