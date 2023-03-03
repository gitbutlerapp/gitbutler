<script lang="ts">
	import type { LayoutData } from './$types';

	export let data: LayoutData;
	$: project = data.project;
</script>

<div class="flex flex-col mt-12">
	<h1 class="text-zinc-200 text-xl flex justify-center">
		Overview of {$project?.title}
	</h1>
	<div class="flex justify-center space-x-2 text-lg">
		<a href="/projects/{$project?.id}/timeline" class="hover:text-zinc-200 text-orange-400"
			>Timeline</a
		>
	</div>
</div>
