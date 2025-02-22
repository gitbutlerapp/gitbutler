<script lang="ts">
	import Button from '@gitbutler/ui/Button.svelte';

	type FormattingAction =
		| 'bold'
		| 'italic'
		| 'underline'
		| 'strikethrough'
		| 'code'
		| 'link'
		| 'quote'
		| 'normal-text'
		| 'h1'
		| 'h2'
		| 'h3'
		| 'bullet-list'
		| 'number-list'
		| 'checkbox-list';
	interface Props {
		onClick: (action: FormattingAction) => void;
	}

	let { onClick }: Props = $props();

	let optionsRefWidth = $state(0);
	let isScrollToSecondGroup = $state(false);
</script>

<div class="formatting-popup">
	<div class="formatting__options" style:width="{optionsRefWidth}px">
		<div class="formatting__options-wrap" class:scrolled={isScrollToSecondGroup}>
			<div class="formatting__group" bind:clientWidth={optionsRefWidth}>
				<Button kind="solid" size="tag" icon="text-bold" onclick={() => onClick('bold')} />
				<Button kind="solid" size="tag" icon="text-italic" onclick={() => onClick('italic')} />
				<Button
					kind="solid"
					size="tag"
					icon="text-underline"
					onclick={() => onClick('underline')}
				/>
				<Button
					kind="solid"
					size="tag"
					icon="text-strikethrough"
					onclick={() => onClick('strikethrough')}
				/>
				<Button kind="solid" size="tag" icon="text-code" onclick={() => onClick('code')} />
				<Button kind="solid" size="tag" icon="text-quote" onclick={() => onClick('quote')} />
				<Button kind="solid" size="tag" icon="text-link" onclick={() => onClick('link')} />
			</div>
			<div class="formatting__group">
				<Button kind="solid" size="tag" icon="text" onclick={() => onClick('normal-text')} />
				<Button kind="solid" size="tag" icon="text-h1" onclick={() => onClick('h1')} />
				<Button kind="solid" size="tag" icon="text-h2" onclick={() => onClick('h2')} />
				<Button kind="solid" size="tag" icon="text-h3" onclick={() => onClick('h3')} />
				<Button kind="solid" size="tag" icon="bullet-list" onclick={() => onClick('bullet-list')} />
				<Button kind="solid" size="tag" icon="number-list" onclick={() => onClick('number-list')} />
				<Button kind="solid" size="tag" icon="checklist" onclick={() => onClick('checkbox-list')} />
			</div>
		</div>
	</div>
	<div class="formatting__next">
		<Button
			kind="solid"
			size="tag"
			tooltip="More options"
			tooltipDelay={1200}
			icon={isScrollToSecondGroup ? 'chevron-left' : 'chevron-right'}
			onclick={() => {
				isScrollToSecondGroup = !isScrollToSecondGroup;
			}}
		/>
	</div>
</div>

<style lang="postcss">
	.formatting-popup {
		display: flex;
		border-radius: var(--radius-ml);
		background-color: var(--clr-theme-ntrl-element);
		box-shadow: var(--shadow-m);
		border: 1px solid var(--clr-border-2);
		margin: 10px;
		box-shadow: var(--fx-shadow-m);
		width: fit-content;
	}

	.formatting__group,
	.formatting__next {
		display: flex;
		gap: 2px;
		padding: 6px;
		width: fit-content;
	}

	.formatting__next {
		position: relative;
		&:before {
			position: absolute;
			top: 0;
			left: 0;
			content: '';
			display: block;
			width: 1px;
			height: 100%;
			background-color: var(--clr-border-2);
			opacity: 0.3;
		}
	}

	.formatting__options {
		position: relative;
		overflow: hidden;
		flex-wrap: nowrap;
	}

	.formatting__options-wrap {
		display: flex;
		transition: transform 0.18s ease;

		&.scrolled {
			transform: translateX(calc(-100% - 1px));
		}
	}
</style>
