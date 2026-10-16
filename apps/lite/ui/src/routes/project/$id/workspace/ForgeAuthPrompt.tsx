import type { ForgeDestination } from "#ui/forge.ts";
import { Button } from "@gitbutler/ui-react/Button.tsx";
import { EmptyState } from "@gitbutler/ui-react/EmptyState.tsx";
import { useAppDispatch } from "#ui/store.ts";
import { interfaceSlice } from "#ui/interface/state.ts";
import type { FC } from "react";
import styles from "./ForgeAuthPrompt.module.css";

type Props = {
	destination: ForgeDestination | null;
	hasAccount: boolean;
};

export const ForgeAuthPrompt: FC<Props> = ({ destination, hasAccount }) => {
	const dispatch = useAppDispatch();

	const title = `${hasAccount ? "Reconnect" : "Connect"} ${destination?.label ?? "forge"}`;
	const description = `${hasAccount ? "Reconnect your account" : "Connect an account"} to view and manage PRs`;

	return (
		<div className={styles.empty}>
			<EmptyState title={title} description={description}>
				<Button
					onClick={() =>
						dispatch(
							interfaceSlice.actions.openDialog({
								dialog: { _tag: "Settings", page: "global:integrations" },
							}),
						)
					}
				>
					{title}
				</Button>
			</EmptyState>
		</div>
	);
};
