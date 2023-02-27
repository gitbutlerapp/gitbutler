<script lang="ts">
	import type { PageData } from './$types';

	export let data: PageData;
	const { project } = data;
</script>

<div class="flex flex-col mt-12">
	<h1 class="text-zinc-200 text-xl flex justify-center">
		Overview of {$project?.title}
	</h1>
	<div class="flex justify-center space-x-2">
		<a class="hover:text-zinc-200" href="/projects/{$project?.id}/week">Week</a>
		<a href="/projects/{$project?.id}/day" class="hover:text-zinc-200">Day</a>
		<a href="/projects/{$project?.id}/timeline" class="hover:text-zinc-200">New timeline</a>
	</div>
</div>
