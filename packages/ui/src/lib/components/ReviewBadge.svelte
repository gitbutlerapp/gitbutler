<script lang="ts">
	import Badge from "$components/Badge.svelte";

	interface Props {
		type: string | undefined;
		status?: "open" | "closed" | "draft" | "merged" | "unknown";
		number: number;
		title?: string;
		testId?: string;
	}

	const { type, status, number, title, testId }: Props = $props();

	const reviewUnit = $derived(type === "MR" ? "MR" : "PR");
	const reviewSymbol = $derived(reviewUnit === "MR" ? "!" : "#");
	const id = $derived(`${reviewSymbol}${number}`);

	function getBadgeStyle(status: Props["status"]): "safe" | "danger" | "purple" | "gray" {
		switch (status) {
			case "open":
				return "safe";
			case "closed":
				return "danger";
			case "merged":
				return "purple";
			default:
				return "gray";
		}
	}

	const badgeDetails = $derived.by(() => {
		if (title) {
			return title;
		}

		switch (status) {
			case "open":
				return `${reviewUnit} ${id} is open`;
			case "closed":
				return `${reviewUnit} ${id} is closed`;
			case "draft":
				return `${reviewUnit} ${id} is a draft`;
			case "merged":
				return `${reviewUnit} ${id} is merged`;
			default:
				return `${reviewUnit} ${id}`;
		}
	});
</script>

<Badge
	{testId}
	tooltip={badgeDetails}
	style={getBadgeStyle(status)}
	kind="soft"
	icon={type === "MR" ? "gitlab" : type === "PR" ? "github" : undefined}
	reversedDirection
>
	{#if status === "draft"}
		Draft {reviewUnit}
	{:else}
		{reviewUnit} {id}
	{/if}
</Badge>
