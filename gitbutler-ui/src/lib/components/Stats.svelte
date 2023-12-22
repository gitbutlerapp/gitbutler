<script lang="ts">
	export let added = 0;
	export let removed = 0;

	$: diffStatRatio = added / (added + removed);
	$: diffStatAdded = Math.floor(4 * diffStatRatio);
	$: diffStatRemoved = 4 - diffStatAdded;
</script>

<div class="flex items-center gap-2">
	<div id="diffstat-lines" class="flex items-center gap-1">
		<span class="text-green-400">+{added}</span>
		<span class="text-red-500">-{removed}</span>
	</div>
	<div id="diffstat-squares" class="flex items-center gap-0.5">
		{#each { length: diffStatAdded } as _}
			<div class="h-2 w-2 rounded-sm bg-green-400" />
		{/each}
		{#each { length: diffStatRemoved } as _}
			<div class="h-2 w-2 rounded-sm bg-red-500" />
		{/each}
	</div>
</div>
