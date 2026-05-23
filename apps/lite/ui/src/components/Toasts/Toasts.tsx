import { Toast } from "@base-ui/react";
import { FC } from "react";
import { classes } from "#ui/components/classes.ts";
import styles from "./Toasts.module.css";
import uiStyles from "#ui/components/ui.module.css";

export const Toasts: FC = () => {
	const { toasts } = Toast.useToastManager();

	return (
		<Toast.Portal>
			<Toast.Viewport className={styles.viewport}>
				{toasts.map((toast) => (
					<Toast.Root key={toast.id} toast={toast} className={classes(uiStyles.popup, styles.root)}>
						<Toast.Content>
							<Toast.Title />
							<Toast.Description
								render={
									// Default is `p` which restricts content elements.
									<div />
								}
							/>
							<Toast.Close>Dismiss</Toast.Close>
						</Toast.Content>
					</Toast.Root>
				))}
			</Toast.Viewport>
		</Toast.Portal>
	);
};
