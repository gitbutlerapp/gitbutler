<script lang="ts">
	interface Props {
		indentLevel: number;
	}

	const { indentLevel }: Props = $props();
</script>

<div class="filetree-indent">
	<span>
		{indentLevel}
	</span>
</div>
