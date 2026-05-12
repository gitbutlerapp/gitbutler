import { AlertDialog } from "@base-ui/react";
import { useEffect, useState } from "react";
import type { FC } from "react";
import { classes } from "#ui/ui/classes.ts";
import uiStyles from "#ui/ui/ui.module.css";

export const Updater: FC = () => {
	const [version, setVersion] = useState<string | undefined>(undefined);

	useEffect(() => {
		const unsubscribeDownloaded = window.lite.onUpdateDownloaded((info) => {
			setVersion(info.version);
		});

		return () => {
			unsubscribeDownloaded();
		};
	}, []);

	return (
		<AlertDialog.Root
			open={version !== undefined}
			onOpenChange={() => {
				// We do not allow dismissing this dialog at this time
			}}
		>
			<AlertDialog.Portal>
				<AlertDialog.Backdrop />
				<AlertDialog.Popup className={classes(uiStyles.popup, uiStyles.dialogPopup)}>
					<AlertDialog.Title>Update {version} downloaded</AlertDialog.Title>
					<AlertDialog.Description>
						Restart to update. You don't have a choice :)
					</AlertDialog.Description>
					<div>
						<button
							className={classes(uiStyles.button)}
							type="button"
							onClick={() => void window.lite.quitAndInstallUpdate()}
						>
							Restart and install
						</button>
					</div>
				</AlertDialog.Popup>
			</AlertDialog.Portal>
		</AlertDialog.Root>
	);
};
