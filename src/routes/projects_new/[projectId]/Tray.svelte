<script lang="ts">
	import { flip } from 'svelte/animate';
	import { quintOut } from 'svelte/easing';
	import { crossfade } from 'svelte/transition';
	import { Checkbox } from '$lib/components';
	import type { Branch } from './types';
	import animateHeight from './animation';

	export let columns: Branch[];

	const [send, receive] = crossfade({
		duration: (d) => {
			return Math.sqrt(d * 500);
		},

		fallback(node, params) {
			const style = getComputedStyle(node);
			const transform = style.transform === 'none' ? '' : style.transform;

			return {
				duration: 600,
				easing: quintOut,
				css: (t) => `
					transform: ${transform} scale(${t});
					opacity: ${t}
				`
			};
		}
	});
</script>

<section class="flex h-full w-64 flex-col gap-y-2 border-r border-zinc-700 bg-[#2F2F33] p-4">
	<div>
		<div use:animateHeight class="flex flex-col gap-y-2 py-2">
			<p>In the working directory:</p>
			{#each columns.filter((c) => c.active) as column (column.id)}
				<div
					in:receive={{ key: column.id }}
					out:send={{ key: column.id }}
					animate:flip={{ duration: 300 }}
					class="rounded border border-zinc-600 bg-zinc-700 p-2"
				>
					<Checkbox bind:checked={column.active} />
					<span class="ml-2">{column.name}</span>
				</div>
			{/each}
		</div>
	</div>
	<div>
		<div use:animateHeight class="flex flex-col gap-y-2 py-2">
			<p>Inactive:</p>
			{#each columns.filter((c) => !c.active) as column (column.id)}
				<div
					in:receive={{ key: column.id }}
					out:send={{ key: column.id }}
					animate:flip={{ duration: 300 }}
					class="rounded border border-zinc-600 bg-zinc-700 p-2"
				>
					<Checkbox bind:checked={column.active} />
					<span class="ml-2">{column.name}</span>
				</div>
			{/each}
		</div>
	</div>
</section>
