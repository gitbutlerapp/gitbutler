<script lang="ts">
	import { formatDistanceToNow } from 'date-fns';
	import type { Commit } from '$lib/vbranches';

	export let commit: Commit;
</script>

<div
	class="flex-grow rounded border border-light-300 bg-light-50 p-2 dark:border-dark-600 dark:bg-dark-900"
>
	<div>
		{commit.description}
	</div>
	<div class="flex text-light-700">
		<div class="flex-grow">{commit.author.name}</div>
		<div>{formatDistanceToNow(commit.createdAt)} ago</div>
	</div>
</div>
