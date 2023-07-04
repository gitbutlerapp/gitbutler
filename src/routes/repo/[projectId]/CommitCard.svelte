<script lang="ts">
	import { formatDistanceToNow } from 'date-fns';
	import type { Commit } from '$lib/api/ipc/vbranches';

	export let commit: Commit;
</script>

<div
	class="flex-grow rounded-lg border border-light-400 bg-white p-2 dark:border-dark-600 dark:bg-dark-1000"
>
	<div>
		{commit.description}
	</div>
	<div class="flex text-light-700">
		<div class="flex-grow">{commit.authorName}</div>
		<div>{formatDistanceToNow(commit.createdAt)} ago</div>
	</div>
</div>
