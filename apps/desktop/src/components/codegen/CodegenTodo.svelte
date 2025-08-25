<script lang="ts">
	import { Icon } from '@gitbutler/ui';
	import iconsJson from '@gitbutler/ui/data/icons.json';
	import type { ClaudeTodo } from '$lib/codegen/types';

	type Props = {
		todo: ClaudeTodo;
	};
	const { todo }: Props = $props();
	const iconName = $derived.by<keyof typeof iconsJson>(() => {
		switch (todo.status) {
			case 'completed':
				return 'circled-checked';
			case 'pending':
				return 'circled-unchecked';
			case 'in_progress':
				return 'running-man';
		}
	});
</script>

<div class="todo clr-text-2">
	<Icon name={iconName} size={14} color={todo.status === 'completed' ? 'success' : undefined} />
	<p
		class="text-12"
		class:clr-text-1={todo.status === 'pending'}
		class:todo-strikethrough={todo.status === 'completed'}
	>
		{todo.content}
	</p>
</div>

<style lang="post-css">
	.todo {
		display: flex;
		width: 100%;
		min-height: 15px;
		padding: 6px 14px;
		gap: 9px;
	}

	.todo-strikethrough {
		text-decoration: strikethrough;
	}
</style>
