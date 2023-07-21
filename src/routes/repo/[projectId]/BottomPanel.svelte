<script lang="ts">
	import { slide } from 'svelte/transition';
	import { IconTriangleUp, IconTriangleDown } from '$lib/icons';
	let shown = false;
</script>

<div class="flex border-t border-light-400 dark:border-dark-600">
	<div class="ml-4 flex flex-col">
		<div
			role="button"
			tabindex="0"
			class="flex h-[20px] items-center gap-2 text-light-700 hover:text-light-900 dark:text-dark-200 dark:hover:text-dark-100"
			on:click={() => (shown = !shown)}
			on:keypress={() => (shown = !shown)}
		>
			{#if shown}
				<IconTriangleDown />
			{:else}
				<IconTriangleUp />
			{/if}
			<span class="text-sm font-bold uppercase"> Common base </span>
		</div>
		{#if shown}
			<div class="h-64 py-2" transition:slide={{ duration: 150 }}>
				<div class="">
					<div>We're no strangers to love You know the rules and so do I</div>
					<div>
						A full commitment's what I'm thinking of You wouldn't get this from any other guy
					</div>
					<div>I just wanna tell you how I'm feeling Gotta make you understand</div>
					<div>Never gonna give you up</div>
					<div>Never gonna let you down</div>
					<div>Never gonna run around and desert you</div>
					<div>Never gonna make you cry</div>
					<div>Never gonna say goodbye</div>
					<div>Never gonna tell a lie and hurt you</div>
				</div>
			</div>
		{/if}
	</div>
</div>
