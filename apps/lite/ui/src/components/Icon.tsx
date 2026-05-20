import { FC, useLayoutEffect, useRef } from "react";
import type { IconName } from "./iconNames";

const svgModules = import.meta.glob("./icons/*.svg", {
	query: "?raw",
	import: "default",
}) as Record<string, () => Promise<string>>;

const icons: Record<string, () => Promise<string>> = {};
for (const [path, loadIcon] of Object.entries(svgModules)) {
	const name = path.replace(/^.*\/(.+)\.svg$/, "$1");
	icons[name] = loadIcon;
}

const iconCache: Record<string, string> = {};
const iconLoadCache: Record<string, Promise<string | undefined> | undefined> = {};

function getIconMarkup(name: IconName): Promise<string | undefined> {
	const cachedIcon = iconCache[name];
	if (cachedIcon !== undefined) return Promise.resolve(cachedIcon);

	const loadIcon = icons[name];
	if (!loadIcon) return Promise.resolve(undefined);

	const pending = iconLoadCache[name];
	if (pending !== undefined) return pending;

	const promise = loadIcon()
		.then((svgMarkup) => {
			iconCache[name] = svgMarkup;
			iconLoadCache[name] = undefined;
			return svgMarkup;
		})
		.catch((err) => {
			iconLoadCache[name] = undefined;
			// oxlint-disable-next-line no-console
			console.error(`Failed to load icon "${name}":`, err);
			return undefined;
		});

	iconLoadCache[name] = promise;
	return promise;
}

type Props = {
	name: IconName;
	size?: number;
};

export const Icon: FC<Props> = ({ name, size = 16 }) => {
	const ref = useRef<HTMLElement>(null);

	useLayoutEffect(() => {
		const node = ref.current;
		if (!node) return;

		let cancelled = false;
		void getIconMarkup(name).then((svgMarkup) => {
			if (cancelled || ref.current !== node) return;
			node.innerHTML = svgMarkup ?? "";
		});

		return () => {
			cancelled = true;
		};
	}, [name]);

	return (
		<i
			ref={ref}
			className="icon"
			aria-hidden="true"
			style={{ display: "inline-flex", width: size, height: size }}
		/>
	);
};
