export type ThemedImageAsset = {
	type: "themed-image";
	lightSrc: string;
	darkSrc: string;
	alt: string;
	width?: string;
	height?: string;
	className?: string;
};

export function isThemedImageAsset(value: unknown): value is ThemedImageAsset {
	return (
		!!value && typeof value === "object" && (value as ThemedImageAsset).type === "themed-image"
	);
}
