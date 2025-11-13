<script lang="ts">
	import Icon from '$components/Icon.svelte';
	import { focusable } from '$lib/focus/focusable';
	import { pxToRem } from '$lib/utils/pxToRem';
	import type { BaseInputProps, InputStylingProps } from '$components/inputTypes';

	export interface Tag {
		id: string;
		label: string;
	}

	interface Props extends BaseInputProps, InputStylingProps {
		tags?: Tag[];
		value?: string;
		maxTags?: number;
		onAddTag?: (tag: Tag) => void;
		onRemoveTag?: (tagId: string) => void;
		onTagsChange?: (tags: Tag[]) => void;
	}

	let {
		id,
		testId,
		label,
		tags = $bindable([]),
		value = $bindable(''),
		placeholder = 'Add tags (split by space/comma)',
		disabled = false,
		readonly = false,
		autofocus: _autofocus = false,
		error,
		helperText,
		wide = false,
		width,
		maxTags,
		onAddTag,
		onRemoveTag,
		onTagsChange
	}: Props = $props();

	let inputEl: HTMLInputElement;
	let hasError = $derived(!!error);

	function generateId(): string {
		return `tag-${Date.now()}-${Math.random().toString(36).substring(2, 9)}`;
	}

	function addTag(label: string) {
		const trimmedLabel = label.trim();

		if (!trimmedLabel) return;
		if (maxTags && tags.length >= maxTags) return;
		if (tags.some((t) => t.label === trimmedLabel)) return;

		const newTag: Tag = {
			id: generateId(),
			label: trimmedLabel
		};

		tags = [...tags, newTag];
		value = '';

		onAddTag?.(newTag);
		onTagsChange?.(tags);
	}

	function removeTag(tagId: string) {
		tags = tags.filter((t) => t.id !== tagId);
		onRemoveTag?.(tagId);
		onTagsChange?.(tags);
		inputEl?.focus();
	}

	function handleKeyDown(e: KeyboardEvent & { currentTarget: EventTarget & HTMLInputElement }) {
		// Add tag on comma or space
		if (e.key === ',' || e.key === ' ') {
			e.preventDefault();
			addTag(value);
		} else if (e.key === 'Backspace' && !value && tags.length > 0) {
			// Remove last tag on backspace when input is empty
			removeTag(tags[tags.length - 1].id);
		}
	}

	function handleBlur() {
		// Optionally add tag on blur if there's content
		if (value.trim()) {
			addTag(value);
		}
	}

	export function focus() {
		inputEl?.focus();
	}
</script>

<div
	class="tag-input-wrapper"
	class:wide
	class:error={hasError}
	style:width={width ? `${pxToRem(width)}rem` : undefined}
>
	{#if label}
		<label class="tag-input-label text-13 text-semibold" for={id}>
			{label}
		</label>
	{/if}

	<div
		class="tag-input-container text-input"
		class:disabled
		class:readonly
		class:error={hasError}
		role="button"
		tabindex="-1"
		onclick={() => !disabled && !readonly && inputEl?.focus()}
		onkeydown={(e) => {
			if (e.key === 'Enter' || e.key === ' ') {
				e.preventDefault();
				inputEl?.focus();
			}
		}}
	>
		<div class="tag-list">
			{#each tags as tag (tag.id)}
				<div class="tag">
					<span class="tag-label text-12">{tag.label}</span>
					{#if !disabled && !readonly}
						<button
							type="button"
							class="tag-remove focus-state"
							onclick={(e) => {
								e.stopPropagation();
								removeTag(tag.id);
							}}
							aria-label="Remove tag"
						>
							<Icon name="cross-small" />
						</button>
					{/if}
				</div>
			{/each}

			<input
				bind:this={inputEl}
				use:focusable={{ button: true }}
				{id}
				data-testid={testId}
				type="text"
				class="tag-input text-13"
				class:disabled
				class:readonly
				{placeholder}
				{disabled}
				{readonly}
				bind:value
				onkeydown={handleKeyDown}
				onblur={handleBlur}
			/>
		</div>
	</div>

	{#if error}
		<p class="text-11 text-body tag-input-error">{error}</p>
	{:else if helperText}
		<p class="text-11 text-body tag-input-helper">{helperText}</p>
	{/if}
</div>

<style lang="postcss">
	.tag-input-wrapper {
		display: flex;
		flex-direction: column;
		gap: 6px;

		&.wide {
			flex: 1;
			width: 100%;
		}
	}

	.tag-input-container {
		min-height: var(--size-cta);
		padding: 4px;
		cursor: text;

		&:focus-within {
			border-color: var(--clr-border-1);
		}

		&.disabled {
			cursor: default;
		}

		&.readonly {
			cursor: default;
		}
	}

	.tag-list {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 2px;
	}

	.tag {
		display: inline-flex;
		align-items: center;
		padding: 2px 4px 2px 8px;
		gap: 2px;
		border: 1px solid var(--clr-border-2);
		border-radius: 100px;
		background-color: var(--clr-bg-2);
		color: var(--clr-text-1);
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}
	}

	.tag-label {
		max-width: 200px;
		overflow: hidden;
		line-height: 1;
		text-overflow: ellipsis;
		white-space: nowrap;
		user-select: none;
	}

	.tag-remove {
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 100px;
		color: var(--clr-text-3);
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-text-2);
		}
	}

	.tag-input {
		flex: 1;
		min-width: 120px;
		padding: 4px 6px;
		border: none;
		outline: none;
		background: transparent;
		color: var(--clr-text-1);

		&::placeholder {
			color: var(--clr-text-3);
		}

		&:disabled,
		&.disabled {
			cursor: default;
		}

		&.readonly {
			cursor: default;
		}
	}

	.tag-input-label {
		color: var(--clr-scale-ntrl-50);
	}

	.tag-input-helper {
		color: var(--clr-scale-ntrl-50);
	}

	.tag-input-error {
		color: var(--clr-theme-err-element);
	}
</style>
