<script lang="ts">
	import type { LayoutData } from './$types';

	export let data: LayoutData;
	$: project = data.project;
</script>

<div class="mt-12 flex flex-col">
	<h1 class="flex justify-center text-xl text-zinc-200">
		Overview of {$project?.title}
	</h1>
	<div class="flex justify-center space-x-2 text-lg">
		<a href="/projects/{$project?.id}/timeline" class="text-orange-400 hover:text-zinc-200"
			>Timeline</a
		>
		<a href="/projects/{$project?.id}/search" class="text-orange-400 hover:text-zinc-200"
			>search (test)</a
		>
		<a href="/projects/{$project?.id}/player" class="hover:text-zinc-200 text-orange-400">player</a>
	</div>
</div>
