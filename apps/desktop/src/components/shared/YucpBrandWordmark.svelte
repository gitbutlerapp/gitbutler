<script lang="ts">
	import { ThemedImage } from "@gitbutler/ui";
	import { YUCP_LOGO_DARK_SRC, YUCP_LOGO_LIGHT_SRC } from "$lib/branding/yucpLogo";

	type Props = {
		height?: string;
		class?: string;
		decorative?: boolean;
	};

	const { height = "0.875rem", class: className = "", decorative = false }: Props = $props();
</script>

<ThemedImage
	lightSrc={YUCP_LOGO_LIGHT_SRC}
	darkSrc={YUCP_LOGO_DARK_SRC}
	alt="YUCP"
	{height}
	class={className}
	{decorative}
/>
