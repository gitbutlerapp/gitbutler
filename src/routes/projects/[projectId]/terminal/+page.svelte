<script lang="ts">
	import type { PageData } from './$types';
	import { shortPath } from '$lib/paths';
	import Terminal from '$lib/components/Terminal.svelte';

	export let data: PageData;
	const { user, filesStatus } = data;

	function runCommand(command: string) {
		command = command + '\r';
		console.log('command input', command);
		const encodedData = new TextEncoder().encode('\x00' + command);
		//ptyWebSocket.send(encodedData);
	}

	function changeTab(tabName: string) {
		const tabs = document.querySelectorAll('.tab');
		tabs.forEach((tab) => {
			console.log('hide tab', tab);
			tab.classList.add('hidden');
		});
		document.querySelector(`#${tabName}`)?.classList.remove('hidden');
		console.log('show tab', document.querySelector(`#${tabName}`));
	}
</script>

<!-- Actual terminal -->
<div class="flex flex-row w-full h-full">
	<div class="w-80">
		<div>Git Status</div>
		{#if $filesStatus}
			{#each $filesStatus as activity}
				<li class="list-disc">
					{activity.status.slice(0, 1)}
					{shortPath(activity.path)}
				</li>
			{/each}
		{/if}
		<hr />
		<div>Commands</div>
		<ul>
			<li on:click={() => runCommand('git push')}>git push</li>
		</ul>
	</div>
	<div class="w-full h-full">
		<div class="flex flex-row">
			<div class="cursor-pointer p-2 bg-zinc-900 rounded mr-1" on:click={() => changeTab('tab-1')}>
				One
			</div>
			<div class="cursor-pointer p-2 bg-zinc-900 rounded mr-1" on:click={() => changeTab('tab-2')}>
				Two
			</div>
		</div>
		<div class="h-full w-full overflow-auto">
			<div class="tab h-full" id="tab-1">
				<Terminal />
			</div>
			<div class="tab h-full hidden" id="tab-2">
				<Terminal />
			</div>
		</div>
	</div>
</div>
