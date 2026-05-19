import yucpLogoOnDark from "$lib/assets/branding/yucp-logo-on-dark.svg";
import yucpLogoOnLight from "$lib/assets/branding/yucp-logo-on-light.svg";
import type { ThemedImageAsset } from "@gitbutler/ui/utils/themedImage";

export const YUCP_LOGO_LIGHT_SRC = yucpLogoOnLight;
export const YUCP_LOGO_DARK_SRC = yucpLogoOnDark;

export function createYucpLogoBadge(overrides: Partial<ThemedImageAsset> = {}): ThemedImageAsset {
	return {
		type: "themed-image",
		lightSrc: YUCP_LOGO_LIGHT_SRC,
		darkSrc: YUCP_LOGO_DARK_SRC,
		alt: "YUCP",
		width: "0.55rem",
		height: "0.625rem",
		className: "yucp-logo-badge",
		...overrides,
	};
}
