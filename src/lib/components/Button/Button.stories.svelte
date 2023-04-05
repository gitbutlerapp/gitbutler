<script lang="ts">
	import { Meta, Story } from '@storybook/addon-svelte-csf';
	import type { ComponentType } from 'svelte';
	import { IconHome } from '../icons';

	import Button from './Button.svelte';

	const heights = ['basic', 'small'] as const;
	const widths = ['basic', 'full-width'] as const;

	const content = [
		[IconHome, 'Label'],
		[undefined, 'Label'],
		[IconHome, '']
	] as [ComponentType, string][];
</script>

<Meta title="GitButler/Button" component={Button} />

<Story name="Basic Button">
	<div class="flex gap-2">
		{#each widths as width}
			{#each heights as height}
				<div class="flex flex-col gap-2">
					{#each [true, false] as filled}
						{#each [true, false] as outlined}
							{#each content as [icon, label]}
								<div class="flex gap-2">
									{#each [false, true] as loading}
										{#each [false, true] as disabled}
											<Button
												role="basic"
												{filled}
												{disabled}
												{outlined}
												{height}
												{width}
												{icon}
												{loading}
												on:click={() => alert('Clicked!')}
											>
												{label}
											</Button>
										{/each}
									{/each}
								</div>
							{/each}
						{/each}
					{/each}
				</div>
			{/each}
		{/each}
	</div>
</Story>

<Story name="Primary Button">
	<div class="flex gap-2">
		{#each widths as width}
			{#each heights as height}
				<div class="flex flex-col gap-2">
					{#each [true, false] as filled}
						{#each [true, false] as outlined}
							{#each content as [icon, label]}
								<div class="flex gap-2">
									{#each [false, true] as loading}
										{#each [false, true] as disabled}
											<Button
												role="primary"
												{filled}
												{outlined}
												{disabled}
												{height}
												{width}
												{icon}
												{loading}
												on:click={() => alert('Clicked!')}
											>
												{label}
											</Button>
										{/each}
									{/each}
								</div>
							{/each}
						{/each}
					{/each}
				</div>
			{/each}
		{/each}
	</div>
</Story>

<Story name="Destructive Button">
	<div class="flex gap-2">
		{#each widths as width}
			{#each heights as height}
				<div class="flex flex-col gap-2">
					{#each [true, false] as filled}
						{#each [true, false] as outlined}
							{#each content as [icon, label]}
								<div class="flex gap-2">
									{#each [false, true] as loading}
										{#each [false, true] as disabled}
											<Button
												role="destructive"
												{filled}
												{disabled}
												{outlined}
												{height}
												{width}
												{icon}
												{loading}
												on:click={() => alert('Clicked!')}
											>
												{label}
											</Button>
										{/each}
									{/each}
								</div>
							{/each}
						{/each}
					{/each}
				</div>
			{/each}
		{/each}
	</div>
</Story>

<Story name="Basic Link">
	<div class="flex gap-2">
		{#each widths as width}
			{#each heights as height}
				<div class="flex flex-col gap-2">
					{#each [true, false] as filled}
						{#each [true, false] as outlined}
							{#each content as [icon, label]}
								<div class="flex gap-2">
									{#each [false, true] as disabled}
										{#each [false, true] as loading}
											<Button
												href="https://gitbutler.com"
												target="_blank"
												role="basic"
												{outlined}
												{filled}
												{disabled}
												{height}
												{width}
												{icon}
												{loading}
											>
												{label}
											</Button>
										{/each}
									{/each}
								</div>
							{/each}
						{/each}
					{/each}
				</div>
			{/each}
		{/each}
	</div>
</Story>

<Story name="Primary Link">
	<div class="flex gap-2">
		{#each widths as width}
			{#each heights as height}
				<div class="flex flex-col gap-2">
					{#each [true, false] as filled}
						{#each [true, false] as outlined}
							{#each content as [icon, label]}
								<div class="flex gap-2">
									{#each [false, true] as loading}
										{#each [false, true] as disabled}
											<Button
												href="https://gitbutler.com"
												target="_blank"
												role="primary"
												{filled}
												{disabled}
												{outlined}
												{height}
												{width}
												{icon}
												{loading}
											>
												{label}
											</Button>
										{/each}
									{/each}
								</div>
							{/each}
						{/each}
					{/each}
				</div>
			{/each}
		{/each}
	</div>
</Story>

<Story name="Destructive Link">
	<div class="flex gap-2">
		{#each widths as width}
			{#each heights as height}
				<div class="flex flex-col gap-2">
					{#each [true, false] as filled}
						{#each [true, false] as outlined}
							{#each content as [icon, label]}
								<div class="flex gap-2">
									{#each [false, true] as loading}
										{#each [false, true] as disabled}
											<Button
												href="https://gitbutler.com"
												target="_blank"
												role="destructive"
												{filled}
												{disabled}
												{outlined}
												{height}
												{width}
												{icon}
												{loading}
											>
												{label}
											</Button>
										{/each}
									{/each}
								</div>
							{/each}
						{/each}
					{/each}
				</div>
			{/each}
		{/each}
	</div>
</Story>
