import { Kbd } from "#ui/components/Kbd.tsx";
import type { FocusScope } from "#ui/focus-scopes.ts";
import type { ComponentProps, FC } from "react";
import styles from "./FocusScopeKbd.module.css";

type Props = Pick<ComponentProps<typeof Kbd>, "hotkey"> & {
	scope: FocusScope;
};

// oxlint-disable-next-line typescript/no-non-null-assertion
const keyClassName = styles.key!;

export const FocusScopeKbd: FC<Props> = ({ hotkey, scope }) => (
	<>
		{/* Inline style as we can't interpolate a custom property into the selectors. */}
		<style href={`selection-scope-kbd-${scope}`}>{`
			[data-selection-focus-styles="true"]:has(
				[data-focus-scope="${scope}"]:focus-within
			) [data-focus-scope-kbd="${scope}"] .${keyClassName} {
				background-color: var(--fill-gray-bg);
				color: var(--fill-gray-fg);
			}
		`}</style>

		<span data-focus-scope-kbd={scope}>
			<Kbd hotkey={hotkey} className={styles.keys} keyClassName={keyClassName} />
		</span>
	</>
);
