<script lang="ts">
	import type { VCommit } from './[projectId]/types';
	import { formatDistanceToNow } from 'date-fns';

	export let projectId: string;
	export let branchId: string;
	export let commit: VCommit;

	console.log(projectId, branchId);
</script>

<div class="my-2 flex flex-col rounded border p-2">
	<div class="flex flex-row justify-between">
		<div>{commit.message}</div>
		<code>{commit.sha.substring(0, 6)}</code>
	</div>
	<div class="flex flex-row justify-between">
		<div>{formatDistanceToNow(new Date(commit.timestamp * 1000))} ago</div>
		<div>{commit.email}</div>
	</div>
</div>
